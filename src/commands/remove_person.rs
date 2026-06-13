use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::process::Stdio;

use crate::git::{ensure_initialized, ensure_valid_key_selector, keys_dir, repo_gpg, Repo};
use crate::gpg::gpg_needs_msys_paths;
use crate::AppResult;

#[derive(clap::Args)]
pub(crate) struct Options {
    #[arg(value_name = "email-or-fingerprint")]
    keys: Vec<String>,
}

pub(crate) fn run(options: Options) -> AppResult<()> {
    if options.keys.is_empty() {
        return Err("removeperson requires at least one fingerprint, key id, or email".to_string());
    }

    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;

    remove_keys(&repo, &options.keys)?;
    for key in options.keys {
        println!("removed recipient {}", key);
    }

    Ok(())
}

fn remove_keys(repo: &Repo, keys: &[String]) -> AppResult<()> {
    let mut removed_fingerprints = Vec::new();
    for key in keys {
        ensure_valid_key_selector(key)?;
        let fingerprints = matching_fingerprints(repo, key)?;
        if fingerprints.is_empty() {
            if !key_is_absent(repo, key)? {
                delete_key(repo, key)?;
            }
            continue;
        }
        removed_fingerprints.extend(fingerprints);
    }

    removed_fingerprints.sort();
    removed_fingerprints.dedup();
    if !removed_fingerprints.is_empty() {
        rewrite_keyring_without(repo, &removed_fingerprints)?;
    }

    Ok(())
}

fn delete_key(repo: &Repo, key: &str) -> AppResult<()> {
    let secret_and_public = run_delete_command(repo, "--delete-secret-and-public-keys", key)?;
    if secret_and_public.status.success() || key_is_absent(repo, key)? {
        return Ok(());
    }

    let public = run_delete_command(repo, "--delete-keys", key)?;
    if public.status.success() || key_is_absent(repo, key)? {
        return Ok(());
    }

    Err(format!(
        "remove recipient {}: command exited with {}: {}",
        key,
        public.status,
        String::from_utf8_lossy(&public.stderr).trim()
    ))
}

fn run_delete_command(repo: &Repo, action: &str, key: &str) -> AppResult<std::process::Output> {
    let mut command = repo_gpg(repo);
    command
        .arg("--batch")
        .arg("--yes")
        .arg("--pinentry-mode")
        .arg("loopback");
    if gpg_needs_msys_paths() {
        command.arg("--no-autostart");
    }
    command
        .arg(action)
        .arg(key)
        .stdout(Stdio::null())
        .output()
        .map_err(|e| format!("remove recipient {}: failed to run command: {}", key, e))
}

fn rewrite_keyring_without(repo: &Repo, removed_fingerprints: &[String]) -> AppResult<()> {
    let removed = removed_fingerprints
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut retained_keys = Vec::new();
    for fingerprint in all_fingerprints(repo)? {
        if removed.contains(fingerprint.as_str()) {
            continue;
        }
        retained_keys.push(export_public_key(repo, &fingerprint)?);
    }

    let keyring = repo.join(keys_dir());
    if keyring.exists() {
        fs::remove_dir_all(&keyring).map_err(|e| format!("remove {}: {}", keyring.display(), e))?;
    }
    fs::create_dir_all(&keyring).map_err(|e| format!("create {}: {}", keyring.display(), e))?;

    for public_key in retained_keys {
        import_public_key(repo, &public_key)?;
    }

    Ok(())
}

fn all_fingerprints(repo: &Repo) -> AppResult<Vec<String>> {
    let mut command = repo_gpg(repo);
    command.arg("--batch");
    if gpg_needs_msys_paths() {
        command.arg("--no-autostart");
    }
    let output = command
        .arg("--with-colons")
        .arg("--list-keys")
        .output()
        .map_err(|e| format!("list recipients: failed to run command: {}", e))?;
    if !output.status.success() {
        return Ok(Vec::new());
    }

    Ok(public_fingerprints(&output.stdout))
}

fn export_public_key(repo: &Repo, fingerprint: &str) -> AppResult<Vec<u8>> {
    let mut command = repo_gpg(repo);
    command.arg("--batch").arg("--armor");
    if gpg_needs_msys_paths() {
        command.arg("--no-autostart");
    }
    let output = command
        .arg("--export")
        .arg(fingerprint)
        .output()
        .map_err(|e| {
            format!(
                "export recipient {}: failed to run command: {}",
                fingerprint, e
            )
        })?;
    if output.status.success() && !output.stdout.is_empty() {
        return Ok(output.stdout);
    }

    Err(format!("export recipient {} failed", fingerprint))
}

fn import_public_key(repo: &Repo, public_key: &[u8]) -> AppResult<()> {
    let mut command = repo_gpg(repo);
    command
        .arg("--batch")
        .arg("--status-fd")
        .arg("1")
        .arg("--import")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    if gpg_needs_msys_paths() {
        command.arg("--no-autostart");
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("import retained recipient: failed to run command: {}", e))?;
    child
        .stdin
        .as_mut()
        .expect("import stdin should be piped")
        .write_all(public_key)
        .map_err(|e| format!("import retained recipient: write stdin: {}", e))?;
    let output = child
        .wait_with_output()
        .map_err(|e| format!("import retained recipient: wait: {}", e))?;
    if output.status.success() || gpg_import_succeeded(&output.stdout) {
        return Ok(());
    }

    Err(format!(
        "import retained recipient failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

fn gpg_import_succeeded(stdout: &[u8]) -> bool {
    String::from_utf8_lossy(stdout)
        .lines()
        .any(|line| line.starts_with("[GNUPG:] IMPORT_OK "))
}

fn matching_fingerprints(repo: &Repo, key: &str) -> AppResult<Vec<String>> {
    let mut command = repo_gpg(repo);
    command.arg("--batch");
    if gpg_needs_msys_paths() {
        command.arg("--no-autostart");
    }
    let output = command
        .arg("--with-colons")
        .arg("--list-keys")
        .arg(key)
        .output()
        .map_err(|e| format!("list recipient {}: failed to run command: {}", key, e))?;
    if !output.status.success() {
        return Ok(Vec::new());
    }

    Ok(public_fingerprints(&output.stdout))
}

fn public_fingerprints(output: &[u8]) -> Vec<String> {
    let mut fingerprints = Vec::new();
    let mut next_fingerprint_is_primary = false;
    for line in String::from_utf8_lossy(output).lines() {
        let fields = line.split(':').collect::<Vec<_>>();
        match fields.first().copied() {
            Some("pub") => next_fingerprint_is_primary = true,
            Some("fpr") if next_fingerprint_is_primary => {
                if let Some(fingerprint) = fields
                    .get(9)
                    .copied()
                    .filter(|fingerprint| !fingerprint.is_empty())
                {
                    fingerprints.push(fingerprint.to_string());
                }
                next_fingerprint_is_primary = false;
            }
            Some("sub") => next_fingerprint_is_primary = false,
            _ => {}
        }
    }
    fingerprints
}

fn key_is_absent(repo: &Repo, key: &str) -> AppResult<bool> {
    let mut command = repo_gpg(repo);
    command.arg("--batch");
    if gpg_needs_msys_paths() {
        command.arg("--no-autostart");
    }
    let status = command
        .arg("--list-keys")
        .arg(key)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("list recipient {}: failed to run command: {}", key, e))?;
    Ok(!status.success())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Args, FromArgMatches};

    fn command() -> clap::Command {
        Options::augment_args(clap::Command::new("removeperson"))
    }

    #[test]
    fn removeperson_options_parse_help_and_keys() {
        let matches = command()
            .try_get_matches_from(["removeperson", "user@example.com"])
            .unwrap();
        let options = Options::from_arg_matches(&matches).unwrap();
        assert_eq!(options.keys, vec!["user@example.com".to_string()]);
    }

    #[test]
    fn removeperson_options_reject_unknown_flags() {
        assert!(command()
            .try_get_matches_from(["removeperson", "-v"])
            .is_err());
    }

    #[test]
    fn public_fingerprints_parses_fpr_records() {
        assert_eq!(
            public_fingerprints(
                b"pub:u:2048:1:D2805A4182E99FF4:::::::\nfpr:::::::::CE82DD3AFC167295F9132371D2805A4182E99FF4:\nuid:u::::::user1 <user1@gitsecret.io>::::::::::\n",
            ),
            vec!["CE82DD3AFC167295F9132371D2805A4182E99FF4".to_string()]
        );
    }
}
