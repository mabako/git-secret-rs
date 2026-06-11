use std::fs;
use std::path::PathBuf;

use crate::crypto::sha256_file;
use crate::git::{ensure_initialized, recipient_key_ids, repo_gpg, Repo};
use crate::mapping::Mapping;
use crate::paths::{encrypted_path, selected_paths};
use crate::process::CommandExt;
use crate::AppResult;

#[derive(clap::Args)]
pub(crate) struct Options {
    #[arg(
        short = 'c',
        help = "deletes encrypted files before creating new ones."
    )]
    clean_encrypted: bool,
    #[arg(
        short = 'F',
        help = "forces hide to continue if a file to encrypt is missing."
    )]
    continue_missing: bool,
    #[arg(
        short = 'P',
        help = "preserve permissions of unencrypted file in encrypted file."
    )]
    preserve_permissions: bool,
    #[arg(short = 'd', help = "deletes unencrypted files after encryption.")]
    delete_plaintext: bool,
    #[arg(short = 'm', help = "encrypt files only when modified.")]
    modified_only: bool,
    #[arg(value_name = "filename")]
    paths: Vec<PathBuf>,
}

pub(crate) fn run(options: Options) -> AppResult<()> {
    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;
    let recipients = recipient_key_ids(&repo)?;
    if recipients.is_empty() {
        return Err("no recipients configured; run 'git secret tell <key>' first".to_string());
    }

    let mut mapping = Mapping::load(&repo)?;
    let paths = selected_paths(&repo, &mapping, options.paths)?;
    let mut mapping_changed = false;
    for path in paths {
        let input = repo.join(&path);
        let output = encrypted_path(&repo, &path);

        if !input.is_file() {
            if options.continue_missing {
                eprintln!("skipped missing {}", path);
                continue;
            }
            return Err(format!("{} is not a file", path));
        }

        let metadata = input
            .metadata()
            .map_err(|e| format!("read {} metadata: {}", input.display(), e))?;
        let digest = sha256_file(&input)?;
        let stored_digest = mapping
            .entries
            .iter()
            .find(|entry| entry.path == path)
            .map(|entry| entry.sha256.as_str())
            .unwrap_or("");
        if options.modified_only && output.exists() && stored_digest == digest {
            println!("unchanged {}", path);
            continue;
        }

        if options.clean_encrypted && output.exists() {
            fs::remove_file(&output).map_err(|e| format!("remove {}: {}", output.display(), e))?;
        }

        let mut cmd = repo_gpg(&repo);
        cmd.arg("--batch")
            .arg("--yes")
            .arg("--trust-model")
            .arg("always")
            .arg("--encrypt");
        if super::secrets_gpg_armor() {
            cmd.arg("--armor");
        }
        for recipient in &recipients {
            cmd.arg("--recipient").arg(recipient);
        }
        cmd.arg("--output").arg(&output).arg(&input);
        cmd.status_ok(&format!("encrypt {}", path))?;
        if mapping.insert_or_update(path.clone(), digest) {
            mapping_changed = true;
        }
        if options.preserve_permissions {
            fs::set_permissions(&output, metadata.permissions())
                .map_err(|e| format!("set permissions on {}: {}", output.display(), e))?;
        }

        if options.delete_plaintext {
            fs::remove_file(&input).map_err(|e| format!("remove {}: {}", input.display(), e))?;
        }

        println!("encrypted {}", path);
    }

    if mapping_changed {
        mapping.save(&repo)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Args, FromArgMatches};

    fn command() -> clap::Command {
        Options::augment_args(clap::Command::new("hide"))
    }

    #[test]
    fn hide_options_parse_flags_and_paths() {
        let matches = command()
            .try_get_matches_from(["hide", "-c", "-F", "-P", "-d", "-m", "secret.env"])
            .unwrap();
        let options = Options::from_arg_matches(&matches).unwrap();

        assert!(options.clean_encrypted);
        assert!(options.continue_missing);
        assert!(options.preserve_permissions);
        assert!(options.delete_plaintext);
        assert!(options.modified_only);
        assert_eq!(options.paths, vec![PathBuf::from("secret.env")]);
    }

    #[test]
    fn hide_options_reject_removed_force_option() {
        assert!(command().try_get_matches_from(["hide", "--force"]).is_err());
        assert!(command().try_get_matches_from(["hide", "-f"]).is_err());
    }
}
