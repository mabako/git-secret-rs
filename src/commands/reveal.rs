use std::fs;
use std::path::PathBuf;

use crate::git::{ensure_initialized, user_gpg, Repo, UserGpgOptions};
use crate::mapping::Mapping;
use crate::paths::{encrypted_path, selected_paths};
use crate::process::CommandExt;
use crate::AppResult;

#[derive(clap::Args)]
pub(crate) struct Options {
    #[arg(
        short = 'f',
        help = "forces gpg to overwrite existing files without prompt."
    )]
    force: bool,
    #[arg(
        short = 'F',
        help = "forces reveal to continue even if a file fails to decrypt."
    )]
    continue_on_error: bool,
    #[arg(short = 'v', help = "verbose, shows extra information.")]
    verbose: bool,
    #[arg(
        short = 'P',
        help = "preserve permissions of encrypted file in unencrypted file."
    )]
    preserve_permissions: bool,
    #[command(flatten)]
    gpg: UserGpgOptions,
    #[arg(value_name = "filename")]
    paths: Vec<PathBuf>,
}

#[cfg(test)]
impl Options {
    pub(crate) fn parse(args: Vec<String>) -> AppResult<Self> {
        super::parse_options("git secret reveal", args)
    }
}

pub(crate) fn run(options: Options) -> AppResult<()> {
    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;
    let mapping = Mapping::load(&repo)?;
    let paths = selected_paths(&repo, &mapping, options.paths)?;

    for path in paths {
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
        if output.exists() && !options.force {
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
        if options.force {
            command.arg("--yes");
        }
        let result = command
            .arg("--decrypt")
            .arg("--output")
            .arg(&output)
            .arg(&input)
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
        if options.verbose {
            println!("decrypted {} from {}", path, input.display());
        } else {
            println!("decrypted {}", path);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reveal_options_parse_all_supported_flags() {
        let options = Options::parse(vec![
            "-f".to_string(),
            "-F".to_string(),
            "-d".to_string(),
            "keys".to_string(),
            "-v".to_string(),
            "-p".to_string(),
            "secret".to_string(),
            "-P".to_string(),
            "file.txt".to_string(),
        ])
        .unwrap();

        assert!(options.force);
        assert!(options.continue_on_error);
        assert!(options.verbose);
        assert!(options.preserve_permissions);
        assert_eq!(options.gpg.homedir, Some(PathBuf::from("keys")));
        assert_eq!(options.gpg.passphrase, Some("secret".to_string()));
        assert_eq!(options.paths, vec![PathBuf::from("file.txt")]);
    }

    #[test]
    fn reveal_options_reject_removed_long_force_option() {
        assert!(Options::parse(vec!["--force".to_string()]).is_err());
    }
}
