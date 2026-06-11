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
    #[arg(short = 'c')]
    clean_encrypted: bool,
    #[arg(short = 'F')]
    continue_missing: bool,
    #[arg(short = 'P')]
    preserve_permissions: bool,
    #[arg(short = 'd')]
    delete_plaintext: bool,
    #[arg(short = 'm')]
    modified_only: bool,
    #[arg(short = 'h', long = "help")]
    help: bool,
    #[arg(value_name = "file")]
    paths: Vec<PathBuf>,
}

#[cfg(test)]
impl Options {
    pub(crate) fn parse(args: Vec<String>) -> AppResult<Self> {
        super::parse_options("git secret hide", args)
    }
}

pub(crate) fn run(options: Options) -> AppResult<()> {
    if options.help {
        print_help();
        return Ok(());
    }

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

fn print_help() {
    println!(
        "git-secret-hide - encrypts all added files with the public keys in this repo.\n\
\n\
Usage:\n\
  git secret hide [-c] [-F] [-P] [-d] [-m] [-h] [file...]\n\
\n\
Options:\n\
  -c  deletes encrypted files before creating new ones\n\
  -F  forces hide to continue if a file to encrypt is missing\n\
  -P  preserve permissions of unencrypted file in encrypted file\n\
  -d  deletes unencrypted files after encryption\n\
  -m  encrypt files only when modified\n\
  -h  shows this help"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hide_options_parse_flags_and_paths() {
        let options = Options::parse(vec![
            "-c".to_string(),
            "-F".to_string(),
            "-P".to_string(),
            "-d".to_string(),
            "-m".to_string(),
            "secret.env".to_string(),
        ])
        .unwrap();

        assert!(options.clean_encrypted);
        assert!(options.continue_missing);
        assert!(options.preserve_permissions);
        assert!(options.delete_plaintext);
        assert!(options.modified_only);
        assert_eq!(options.paths, vec![PathBuf::from("secret.env")]);
    }

    #[test]
    fn hide_options_reject_removed_force_option() {
        assert!(Options::parse(vec!["--force".to_string()]).is_err());
        assert!(Options::parse(vec!["-f".to_string()]).is_err());
    }
}
