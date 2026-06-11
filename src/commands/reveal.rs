use std::fs;
use std::path::PathBuf;

use crate::git::{ensure_initialized, user_gpg, Repo, UserGpgOptions};
use crate::mapping::Mapping;
use crate::paths::{encrypted_path, selected_paths};
use crate::process::CommandExt;
use crate::AppResult;

pub(crate) struct Options {
    force: bool,
    continue_on_error: bool,
    verbose: bool,
    preserve_permissions: bool,
    help: bool,
    gpg: UserGpgOptions,
    paths: Vec<PathBuf>,
}

impl Options {
    pub(crate) fn parse(args: Vec<String>) -> AppResult<Self> {
        let mut force = false;
        let mut continue_on_error = false;
        let mut verbose = false;
        let mut preserve_permissions = false;
        let mut help = false;
        let mut gpg = UserGpgOptions::default();
        let mut paths = Vec::new();
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-f" => force = true,
                "-F" => continue_on_error = true,
                "-d" => {
                    let homedir = args
                        .next()
                        .ok_or_else(|| "reveal option -d requires a gpg homedir".to_string())?;
                    gpg.homedir = Some(PathBuf::from(homedir));
                }
                "-v" => verbose = true,
                "-p" => {
                    let passphrase = args
                        .next()
                        .ok_or_else(|| "reveal option -p requires a password".to_string())?;
                    gpg.passphrase = Some(passphrase);
                }
                "-P" => preserve_permissions = true,
                "-h" | "--help" => help = true,
                _ if arg.starts_with('-') => {
                    return Err(format!("unknown reveal option '{}'", arg))
                }
                _ => paths.push(PathBuf::from(arg)),
            }
        }

        Ok(Self {
            force,
            continue_on_error,
            verbose,
            preserve_permissions,
            help,
            gpg,
            paths,
        })
    }
}

pub(crate) fn run(options: Options) -> AppResult<()> {
    if options.help {
        print_help();
        return Ok(());
    }

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

fn print_help() {
    println!(
        "git-secret-reveal - decrypts passed files, or all files considered secret by git-secret.\n\
\n\
Usage:\n\
  git secret reveal [-f] [-F] [-d <gpg-homedir>] [-v] [-p <password>] [-P] [-h] [file...]\n\
\n\
Options:\n\
  -f  forces gpg to overwrite existing files without prompt\n\
  -F  forces reveal to continue even if a file fails to decrypt\n\
  -d  specifies --homedir option for gpg\n\
  -v  verbose, shows extra information\n\
  -p  specifies password for noinput mode, adds --passphrase option for gpg\n\
  -P  preserve permissions of encrypted file in unencrypted file\n\
  -h  shows this help"
    );
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
