use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::git::{ensure_initialized, ensure_valid_key_selector, keys_dir, repo_gpg, Repo};
use crate::gpg::{gpg_arg_path, gpg_command, gpg_needs_msys_paths};
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
    for key in keys {
        ensure_valid_key_selector(key)?;
    }

    for key in keys {
        let secret_fingerprints = matching_secret_fingerprints(repo, key)?;
        if !secret_fingerprints.is_empty()
            || (repository_has_secret_key_files(repo) && !key_is_absent(repo, key)?)
        {
            return Err(secret_key_removal_error(repo, key, &secret_fingerprints));
        }
    }

    let mut present_keys = Vec::new();
    for key in keys {
        if !key_is_absent(repo, key)? {
            present_keys.push(key.clone());
        }
    }
    if !present_keys.is_empty() {
        delete_keys(repo, &present_keys)?;
    }

    Ok(())
}

fn secret_key_removal_error(repo: &Repo, key: &str, secret_fingerprints: &[String]) -> String {
    let key = secret_fingerprints
        .first()
        .map(String::as_str)
        .unwrap_or(key);
    format!(
        "recipient '{}' has a secret key in the repository keyring; delete it manually with: {}",
        key,
        manual_delete_secret_key_command(repo, key)
    )
}

fn manual_delete_secret_key_command(repo: &Repo, key: &str) -> String {
    format!(
        "gpg --homedir {} --delete-secret-keys {}",
        shell_quote(&repo.join(keys_dir()).to_string_lossy()),
        shell_quote(key)
    )
}

fn shell_quote(value: &str) -> String {
    if !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-_./:=@\\".contains(character))
    {
        return value.to_string();
    }

    format!("'{}'", value.replace('\'', "'\\''"))
}

fn delete_keys(repo: &Repo, keys: &[String]) -> AppResult<()> {
    let public = run_delete_command(repo, keys)?;
    if public.status.success() || keys_are_absent(repo, keys)? {
        return Ok(());
    }

    if delete_keys_using_short_homedir(repo, keys)? {
        return Ok(());
    }

    let keys = keys.join(", ");
    Err(format!(
        "remove recipient {}: command exited with {}: {}",
        keys,
        public.status,
        String::from_utf8_lossy(&public.stderr).trim()
    ))
}

fn run_delete_command(repo: &Repo, keys: &[String]) -> AppResult<std::process::Output> {
    let mut command = repo_gpg(repo);
    command.arg("--batch").arg("--yes");
    if gpg_needs_msys_paths() {
        command.arg("--no-autostart");
    }
    command
        .arg("--delete-keys")
        .args(keys)
        .stdout(Stdio::null())
        .output()
        .map_err(|e| {
            format!(
                "remove recipient {}: failed to run command: {}",
                keys.join(", "),
                e
            )
        })
}

fn delete_keys_using_short_homedir(repo: &Repo, keys: &[String]) -> AppResult<bool> {
    let source = repo.join(keys_dir());
    let temp = temporary_keyring_path();
    copy_dir(&source, &temp)?;
    let output = run_delete_command_in_homedir(&temp, keys)?;
    let deleted = output.status.success() || keys_are_absent_in_homedir(&temp, keys)?;
    if deleted {
        replace_dir(&source, &temp)?;
        return Ok(true);
    }

    let _ = fs::remove_dir_all(&temp);
    Ok(false)
}

fn run_delete_command_in_homedir(
    homedir: &Path,
    keys: &[String],
) -> AppResult<std::process::Output> {
    let mut command = gpg_command();
    command
        .arg("--homedir")
        .arg(gpg_arg_path(homedir))
        .arg("--batch")
        .arg("--yes")
        .arg("--delete-keys")
        .args(keys)
        .stdout(Stdio::null())
        .output()
        .map_err(|e| {
            format!(
                "remove recipient {}: failed to run command: {}",
                keys.join(", "),
                e
            )
        })
}

fn keys_are_absent(repo: &Repo, keys: &[String]) -> AppResult<bool> {
    for key in keys {
        if !key_is_absent(repo, key)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn keys_are_absent_in_homedir(homedir: &Path, keys: &[String]) -> AppResult<bool> {
    for key in keys {
        if !key_is_absent_in_homedir(homedir, key)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn key_is_absent_in_homedir(homedir: &Path, key: &str) -> AppResult<bool> {
    let status = gpg_command()
        .arg("--homedir")
        .arg(gpg_arg_path(homedir))
        .arg("--batch")
        .arg("--list-keys")
        .arg(key)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("list recipient {}: failed to run command: {}", key, e))?;
    Ok(!status.success())
}

fn temporary_keyring_path() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("gshr-rm-{}-{unique:x}", std::process::id()))
}

fn copy_dir(source: &Path, destination: &Path) -> AppResult<()> {
    fs::create_dir_all(destination)
        .map_err(|e| format!("create {}: {}", destination.display(), e))?;
    for entry in fs::read_dir(source).map_err(|e| format!("read {}: {}", source.display(), e))? {
        let entry = entry.map_err(|e| format!("read {}: {}", source.display(), e))?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir(&source_path, &destination_path)?;
        } else if !is_lock_file(&source_path) {
            fs::copy(&source_path, &destination_path).map_err(|e| {
                format!(
                    "copy {} to {}: {}",
                    source_path.display(),
                    destination_path.display(),
                    e
                )
            })?;
        }
    }
    Ok(())
}

fn replace_dir(target: &Path, replacement: &Path) -> AppResult<()> {
    if target.exists() {
        fs::remove_dir_all(target).map_err(|e| format!("remove {}: {}", target.display(), e))?;
    }
    fs::rename(replacement, target).map_err(|e| {
        format!(
            "replace {} with {}: {}",
            target.display(),
            replacement.display(),
            e
        )
    })
}

fn is_lock_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|file_name| file_name.to_str())
        .is_some_and(|file_name| file_name.ends_with(".lock"))
}

fn repository_has_secret_key_files(repo: &Repo) -> bool {
    let private_keys = repo.join(keys_dir()).join("private-keys-v1.d");
    fs::read_dir(private_keys)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .any(|entry| entry.path().is_file())
}

fn matching_secret_fingerprints(repo: &Repo, key: &str) -> AppResult<Vec<String>> {
    let mut command = repo_gpg(repo);
    command.arg("--batch");
    if gpg_needs_msys_paths() {
        command.arg("--no-autostart");
    }
    let output = command
        .arg("--with-colons")
        .arg("--list-secret-keys")
        .arg(key)
        .output()
        .map_err(|e| {
            format!(
                "list secret recipient {}: failed to run command: {}",
                key, e
            )
        })?;
    if !output.status.success() {
        return Ok(Vec::new());
    }

    Ok(secret_fingerprints(&output.stdout))
}

fn secret_fingerprints(output: &[u8]) -> Vec<String> {
    let mut fingerprints = Vec::new();
    let mut next_fingerprint_is_primary = false;
    for line in String::from_utf8_lossy(output).lines() {
        let fields = line.split(':').collect::<Vec<_>>();
        match fields.first().copied() {
            Some("sec") => next_fingerprint_is_primary = true,
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
            Some("ssb") => next_fingerprint_is_primary = false,
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
    fn secret_fingerprints_parses_secret_primary_fpr_records() {
        assert_eq!(
            secret_fingerprints(
                b"sec:u:2048:1:D2805A4182E99FF4:::::::\nfpr:::::::::CE82DD3AFC167295F9132371D2805A4182E99FF4:\nuid:u::::::user1 <user1@gitsecret.io>::::::::::\nssb:u:2048:1:AAAAAAAAAAAAAAAA:::::::\nfpr:::::::::BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB:\n",
            ),
            vec!["CE82DD3AFC167295F9132371D2805A4182E99FF4".to_string()]
        );
    }
}
