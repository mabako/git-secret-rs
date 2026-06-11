use std::fs;
use std::path::{Path, PathBuf};

use crate::git::{ensure_initialized, user_gpg, Repo, UserGpgOptions};
use crate::mapping::Mapping;
use crate::paths::encrypted_path;
use crate::AppResult;

pub(crate) struct Options {
    gpg: UserGpgOptions,
    help: bool,
}

impl Options {
    pub(crate) fn parse(args: Vec<String>) -> AppResult<Self> {
        let mut gpg = UserGpgOptions::default();
        let mut help = false;
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-d" => {
                    let homedir = args
                        .next()
                        .ok_or_else(|| "changes option -d requires a gpg homedir".to_string())?;
                    gpg.homedir = Some(PathBuf::from(homedir));
                }
                "-p" => {
                    let passphrase = args
                        .next()
                        .ok_or_else(|| "changes option -p requires a password".to_string())?;
                    gpg.passphrase = Some(passphrase);
                }
                "-h" | "--help" => help = true,
                _ => return Err(format!("unknown changes option '{}'", arg)),
            }
        }

        Ok(Self { gpg, help })
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
        .arg(secret)
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

fn print_help() {
    println!(
        "git-secret-changes - shows changes between the current versions of secret files and encrypted versions.\n\
\n\
Usage:\n\
  git secret changes [-d <gpg-homedir>] [-p <password>] [-h]\n\
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
    fn changes_options_parse_homedir_password_and_help() {
        let options = Options::parse(vec![
            "-d".to_string(),
            "keys".to_string(),
            "-p".to_string(),
            "secret".to_string(),
            "-h".to_string(),
        ])
        .unwrap();

        assert_eq!(options.gpg.homedir, Some(PathBuf::from("keys")));
        assert_eq!(options.gpg.passphrase, Some("secret".to_string()));
        assert!(options.help);
    }

    #[test]
    fn changes_options_require_values() {
        assert!(Options::parse(vec!["-d".to_string()]).is_err());
        assert!(Options::parse(vec!["-p".to_string()]).is_err());
    }
}
