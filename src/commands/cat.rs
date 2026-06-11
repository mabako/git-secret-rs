use std::path::PathBuf;

use crate::git::{ensure_initialized, user_gpg, Repo, UserGpgOptions};
use crate::paths::{encrypted_path, normalize_secret_path};
use crate::process::CommandExt;
use crate::AppResult;

pub(crate) struct Options {
    gpg: UserGpgOptions,
    paths: Vec<PathBuf>,
    help: bool,
}

impl Options {
    pub(crate) fn parse(args: Vec<String>) -> AppResult<Self> {
        let mut gpg = UserGpgOptions::default();
        let mut paths = Vec::new();
        let mut help = false;
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-d" => {
                    let homedir = args
                        .next()
                        .ok_or_else(|| "cat option -d requires a gpg homedir".to_string())?;
                    gpg.homedir = Some(PathBuf::from(homedir));
                }
                "-p" => {
                    let passphrase = args
                        .next()
                        .ok_or_else(|| "cat option -p requires a password".to_string())?;
                    gpg.passphrase = Some(passphrase);
                }
                "-h" | "--help" => help = true,
                _ if arg.starts_with('-') => return Err(format!("unknown cat option '{}'", arg)),
                _ => paths.push(PathBuf::from(arg)),
            }
        }

        Ok(Self { gpg, paths, help })
    }
}

pub(crate) fn run(options: Options) -> AppResult<()> {
    if options.help {
        print_help();
        return Ok(());
    }
    if options.paths.is_empty() {
        return Err("cat requires at least one file".to_string());
    }

    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;

    for path in options.paths {
        let normalized = normalize_secret_path(&path)?;
        let secret = encrypted_path(&repo, &normalized);
        user_gpg(&options.gpg)
            .arg("--batch")
            .arg("--decrypt")
            .arg(&secret)
            .status_ok(&format!("decrypt {}", normalized))?;
    }

    Ok(())
}

fn print_help() {
    println!(
        "git-secret-cat - prints decrypted contents of passed files.\n\
\n\
Usage:\n\
  git secret cat [-d <gpg-homedir>] [-p <password>] <file> [file...]\n\
\n\
Options:\n\
  -d  specifies --homedir option for gpg\n\
  -p  specifies password for noinput mode, adds --passphrase option for gpg\n\
  -h  shows this help"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cat_options_parse_homedir_password_and_paths() {
        let options = Options::parse(vec![
            "-d".to_string(),
            "keys".to_string(),
            "-p".to_string(),
            "secret".to_string(),
            "file.txt".to_string(),
        ])
        .unwrap();

        assert_eq!(options.gpg.homedir, Some(PathBuf::from("keys")));
        assert_eq!(options.gpg.passphrase, Some("secret".to_string()));
        assert_eq!(options.paths, vec![PathBuf::from("file.txt")]);
    }

    #[test]
    fn cat_options_parse_help() {
        assert!(Options::parse(vec!["-h".to_string()]).unwrap().help);
    }
}
