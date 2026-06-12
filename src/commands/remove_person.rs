use std::process::Stdio;

use crate::git::{ensure_initialized, gpg_needs_msys_paths, repo_gpg, Repo};
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

    for key in options.keys {
        remove_key(&repo, &key)?;
        println!("removed recipient {}", key);
    }

    Ok(())
}

fn remove_key(repo: &Repo, key: &str) -> AppResult<()> {
    let mut command = repo_gpg(repo);
    command.arg("--batch").arg("--yes");
    if gpg_needs_msys_paths() {
        command.arg("--no-autostart");
    }
    let status = command
        .arg("--delete-keys")
        .arg(key)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("remove recipient {}: failed to run command: {}", key, e))?;
    if status.success() || key_is_absent(repo, key)? {
        Ok(())
    } else {
        Err(format!(
            "remove recipient {}: command exited with {}",
            key, status
        ))
    }
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
}
