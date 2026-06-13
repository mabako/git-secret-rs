use std::fs;
use std::path::PathBuf;

use crate::crypto::sha256_file;
use crate::git::{ensure_initialized, Repo};
use crate::gpg::{gpg_arg_path, user_gpg, UserGpgOptions};
use crate::mapping::{Mapping, MappingEntry};
use crate::paths::{encrypted_path, selected_paths};
use crate::process::CommandExt;
use crate::AppResult;

#[derive(clap::Args)]
pub(crate) struct Options {
    #[arg(
        short = 'a',
        long = "always-decrypt",
        help = "Always decrypt files, ignoring checksums"
    )]
    always_decrypt: bool,
    #[arg(
        short = 'f',
        help = "Forces gpg to overwrite existing files without prompt"
    )]
    force: bool,
    #[arg(
        short = 'F',
        help = "Forces reveal to continue even if a file fails to decrypt"
    )]
    continue_on_error: bool,
    #[arg(short = 'v', help = "Verbose, shows extra information")]
    verbose: bool,
    #[arg(
        short = 'P',
        help = "Preserve permissions of encrypted file in unencrypted file"
    )]
    preserve_permissions: bool,
    #[command(flatten)]
    gpg: UserGpgOptions,
    #[arg(value_name = "filename")]
    paths: Vec<PathBuf>,
}

pub(crate) fn run(options: Options) -> AppResult<()> {
    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;
    let mapping = Mapping::load(&repo)?;
    let paths = selected_paths(&repo, &mapping, options.paths)?;
    let verbose = options.verbose || super::secrets_verbose();

    for path in paths {
        let entry = mapping.entries.iter().find(|entry| entry.path == path);
        let input = encrypted_path(&repo, &path);
        let output = repo.join(&path);

        if !input.is_file() {
            let error = format!("{} does not exist", input.display());
            if options.continue_on_error {
                eprintln!("skipped {}: {}", path, error);
                continue;
            }
            return Err(error);
        }
        if !options.always_decrypt && plaintext_matches_mapping(entry, &output)? {
            if verbose {
                println!("unchanged {} from {}", path, input.display());
            } else {
                println!("unchanged {}", path);
            }
            continue;
        }
        if output.exists() && !(options.force || options.always_decrypt) {
            let error = format!("{} already exists; pass -f to overwrite", output.display());
            if options.continue_on_error {
                eprintln!("skipped {}: {}", path, error);
                continue;
            }
            return Err(error);
        }

        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("create {}: {}", parent.display(), e))?;
        }

        let encrypted_permissions = input
            .metadata()
            .map_err(|e| format!("read {} metadata: {}", input.display(), e))?
            .permissions();
        let mut command = user_gpg(&options.gpg);
        command.arg("--batch");
        if options.force || options.always_decrypt {
            command.arg("--yes");
        }
        let result = command
            .arg("--decrypt")
            .arg("--output")
            .arg(gpg_arg_path(&output))
            .arg(gpg_arg_path(&input))
            .status_ok(&format!("decrypt {}", path));
        if let Err(error) = result {
            if options.continue_on_error {
                eprintln!("skipped {}: {}", path, error);
                continue;
            }
            return Err(error);
        }
        if options.preserve_permissions {
            fs::set_permissions(&output, encrypted_permissions)
                .map_err(|e| format!("set permissions on {}: {}", output.display(), e))?;
        }
        if verbose {
            println!("decrypted {} from {}", path, input.display());
        } else {
            println!("decrypted {}", path);
        }
    }

    Ok(())
}

fn plaintext_matches_mapping(
    entry: Option<&MappingEntry>,
    output: &std::path::Path,
) -> AppResult<bool> {
    let Some(entry) = entry else {
        return Ok(false);
    };
    if entry.sha256.is_empty() || !output.is_file() {
        return Ok(false);
    }

    Ok(sha256_file(output)? == entry.sha256)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Args, FromArgMatches};

    fn command() -> clap::Command {
        Options::augment_args(clap::Command::new("reveal"))
    }

    #[test]
    fn reveal_options_parse_all_supported_flags() {
        let matches = command()
            .try_get_matches_from([
                "reveal", "-a", "-f", "-F", "-d", "keys", "-v", "-p", "secret", "-P", "file.txt",
            ])
            .unwrap();
        let options = Options::from_arg_matches(&matches).unwrap();

        assert!(options.always_decrypt);
        assert!(options.force);
        assert!(options.continue_on_error);
        assert!(options.verbose);
        assert!(options.preserve_permissions);
        assert_eq!(options.gpg.homedir, Some(PathBuf::from("keys")));
        assert_eq!(options.gpg.passphrase, Some("secret".to_string()));
        assert_eq!(options.paths, vec![PathBuf::from("file.txt")]);

        let matches = command()
            .try_get_matches_from(["reveal", "--always-decrypt", "file.txt"])
            .unwrap();
        let options = Options::from_arg_matches(&matches).unwrap();
        assert!(options.always_decrypt);
        assert_eq!(options.paths, vec![PathBuf::from("file.txt")]);
    }

    #[test]
    fn reveal_options_reject_removed_long_force_option() {
        assert!(command()
            .try_get_matches_from(["reveal", "--force"])
            .is_err());
    }
}
