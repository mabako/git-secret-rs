use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::git::{ensure_initialized, ensure_valid_key_selector, repo_gpg, Repo};
use crate::gpg::{gpg_arg_path, gpg_command, gpg_needs_msys_paths};
use crate::AppResult;

#[derive(clap::Args)]
pub(crate) struct Options {
    #[arg(
        short = 'm',
        help = "Uses your current `git config user.email` setting as an identifier for the key"
    )]
    use_git_email: bool,
    #[arg(
        short = 'd',
        value_name = "gpg-homedir",
        help = "Specifies `--homedir` option for `gpg`, basically use this option if your store your keys in a custom location"
    )]
    gpg_homedir: Option<PathBuf>,
    #[arg(value_name = "email-or-fingerprint")]
    keys: Vec<String>,
    #[arg(short = 'v', help = "Verbose mode, accepted for compatibility")]
    verbose: bool,
}

pub(crate) fn run(options: Options) -> AppResult<Vec<String>> {
    let _ = options.verbose;
    let mut keys = options.keys;
    if options.use_git_email {
        keys.push(git_user_email()?);
    }

    if keys.is_empty() {
        return Err("tell requires at least one fingerprint, key id, or email".to_string());
    }

    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;
    let mut imported = Vec::new();

    for key in keys {
        ensure_valid_key_selector(&key)?;
        ensure_unambiguous_public_key(options.gpg_homedir.as_ref(), &key)?;
        ensure_recipient_is_absent(&repo, &key)?;
        let exported = source_gpg(options.gpg_homedir.as_ref())
            .arg("--batch")
            .arg("--armor")
            .arg("--export")
            .arg(&key)
            .output()
            .map_err(|e| format!("run gpg --export {}: {}", key, e))?;
        if !exported.status.success() || exported.stdout.is_empty() {
            return Err(format!("could not export public key '{}'", key));
        }

        import_public_key(&repo, &key, &exported.stdout)?;

        println!("added recipient {}", key);
        imported.push(key);
    }

    Ok(imported)
}

fn ensure_recipient_is_absent(repo: &Repo, key: &str) -> AppResult<()> {
    let status = repo_gpg(repo)
        .arg("--batch")
        .arg("--list-keys")
        .arg(key)
        .status()
        .map_err(|e| format!("check existing recipient {}: {}", key, e))?;
    if status.success() {
        return Err(format!("recipient '{}' already exists", key));
    }

    Ok(())
}

fn ensure_unambiguous_public_key(homedir: Option<&PathBuf>, key: &str) -> AppResult<()> {
    let output = source_gpg(homedir)
        .arg("--batch")
        .arg("--with-colons")
        .arg("--list-keys")
        .arg(key)
        .output()
        .map_err(|e| format!("run gpg --list-keys {}: {}", key, e))?;
    if !output.status.success() {
        return Ok(());
    }

    let matches = public_key_count(&output.stdout);
    if matches > 1 {
        return Err(format!(
            "multiple public keys match '{}'; specify a fingerprint or key id",
            key
        ));
    }

    Ok(())
}

fn public_key_count(output: &[u8]) -> usize {
    String::from_utf8_lossy(output)
        .lines()
        .filter(|line| line.starts_with("pub:"))
        .count()
}

fn source_gpg(homedir: Option<&PathBuf>) -> Command {
    let mut command = gpg_command();
    if let Some(homedir) = homedir {
        command.arg("--homedir").arg(gpg_arg_path(homedir));
    }
    command
}

fn import_public_key(repo: &Repo, key: &str, public_key: &[u8]) -> AppResult<()> {
    let key_file = temporary_public_key_path();
    fs::write(&key_file, public_key).map_err(|e| format!("write {}: {}", key_file.display(), e))?;

    let mut command = repo_gpg(repo);
    command.arg("--batch");
    if gpg_needs_msys_paths() {
        command.arg("--no-autostart");
    }
    let output = command
        .arg("--status-fd")
        .arg("1")
        .arg("--import")
        .arg(gpg_arg_path(&key_file))
        .output()
        .map_err(|e| format!("run gpg --import: {}", e));

    let _ = fs::remove_file(&key_file);
    let output = output?;
    if output.status.success() || gpg_import_succeeded(&output.stdout) {
        Ok(())
    } else {
        Err(format!(
            "could not import public key '{}': {}",
            key,
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn gpg_import_succeeded(stdout: &[u8]) -> bool {
    String::from_utf8_lossy(stdout)
        .lines()
        .any(|line| line.starts_with("[GNUPG:] IMPORT_OK "))
}

fn temporary_public_key_path() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "git-secret-tell-{}-{unique:x}.asc",
        std::process::id()
    ))
}

fn git_user_email() -> AppResult<String> {
    let output = Command::new("git")
        .arg("config")
        .arg("user.email")
        .output()
        .map_err(|e| format!("run git config user.email: {}", e))?;
    if !output.status.success() {
        return Err("could not read git config user.email".to_string());
    }

    let email = String::from_utf8(output.stdout)
        .map_err(|_| "git config user.email returned non-UTF-8 output".to_string())?
        .trim()
        .to_string();
    if email.is_empty() {
        return Err("git config user.email is empty".to_string());
    }

    Ok(email)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Args, FromArgMatches};

    fn command() -> clap::Command {
        Options::augment_args(clap::Command::new("tell"))
    }

    #[test]
    fn tell_options_parse_git_email_and_homedir() {
        let matches = command()
            .try_get_matches_from(["tell", "-m", "-v", "-d", "keys", "user@example.com"])
            .unwrap();
        let options = Options::from_arg_matches(&matches).unwrap();

        assert!(options.use_git_email);
        assert!(options.verbose);
        assert_eq!(options.gpg_homedir, Some(PathBuf::from("keys")));
        assert_eq!(options.keys, vec!["user@example.com".to_string()]);
    }

    #[test]
    fn tell_options_parse_help() {
        assert!(command().try_get_matches_from(["tell", "-h"]).is_err());
    }

    #[test]
    fn tell_options_require_homedir_after_d() {
        assert!(command().try_get_matches_from(["tell", "-d"]).is_err());
    }

    #[test]
    fn public_key_count_counts_primary_keys() {
        assert_eq!(
            public_key_count(
                b"tru:o:1:0:0:0:0:0\npub:u:2048:1:AAAAAAAAAAAAAAAA:::::::\nuid:u::::::User One::::::::\nsub:u:2048:1:BBBBBBBBBBBBBBBB:::::::\npub:u:2048:1:CCCCCCCCCCCCCCCC:::::::\n",
            ),
            2
        );
    }
}
