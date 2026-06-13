use std::fs;
use std::path::Path;

use crate::git::{ensure_initialized, Repo};
use crate::gpg::{gpg_arg_path, user_gpg, UserGpgOptions};
use crate::mapping::Mapping;
use crate::paths::encrypted_path;
use crate::AppResult;

#[derive(clap::Args)]
pub(crate) struct Options {
    #[command(flatten)]
    gpg: UserGpgOptions,
}

pub(crate) fn run(options: Options) -> AppResult<()> {
    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;
    let mapping = Mapping::load(&repo)?;
    let mut changed = false;

    for entry in mapping.entries {
        let plaintext = repo.join(&entry.path);
        let secret = encrypted_path(&repo, &entry.path);
        let status = file_change_status(&options.gpg, &plaintext, &secret)?;
        if let Some(status) = status {
            println!("{}\t{}", status, entry.path);
            changed = true;
        }
    }

    if !changed {
        println!("no changes");
    }

    Ok(())
}

fn file_change_status(
    gpg: &UserGpgOptions,
    plaintext: &Path,
    secret: &Path,
) -> AppResult<Option<&'static str>> {
    if !plaintext.exists() {
        return Ok(None);
    }
    if !secret.exists() {
        return Ok(Some("new"));
    }

    let plaintext = fs::read(plaintext)
        .map_err(|e| format!("read plaintext {}: {}", plaintext.display(), e))?;
    let decrypted = decrypt_secret(gpg, secret)?;
    if plaintext != decrypted {
        Ok(Some("modified"))
    } else {
        Ok(None)
    }
}

fn decrypt_secret(gpg: &UserGpgOptions, secret: &Path) -> AppResult<Vec<u8>> {
    let output = user_gpg(gpg)
        .arg("--batch")
        .arg("--decrypt")
        .arg(gpg_arg_path(secret))
        .output()
        .map_err(|e| format!("decrypt {}: {}", secret.display(), e))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(format!(
            "decrypt {}: gpg exited with {}",
            secret.display(),
            output.status
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Args, FromArgMatches};
    use std::path::PathBuf;

    fn command() -> clap::Command {
        Options::augment_args(clap::Command::new("changes"))
    }

    #[test]
    fn changes_options_parse_homedir_password_and_help() {
        let matches = command()
            .try_get_matches_from(["changes", "-d", "keys", "-p", "secret"])
            .unwrap();
        let options = Options::from_arg_matches(&matches).unwrap();

        assert_eq!(options.gpg.homedir, Some(PathBuf::from("keys")));
        assert_eq!(options.gpg.passphrase, Some("secret".to_string()));
    }

    #[test]
    fn changes_options_require_values() {
        assert!(command().try_get_matches_from(["changes", "-d"]).is_err());
        assert!(command().try_get_matches_from(["changes", "-p"]).is_err());
    }
}
