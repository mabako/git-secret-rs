use std::path::PathBuf;

use crate::git::{user_gpg, UserGpgOptions};
use crate::process::CommandExt;
use crate::AppResult;

pub(crate) struct Options {
    gpg: UserGpgOptions,
    paths: Vec<PathBuf>,
}

impl Options {
    pub(crate) fn parse(args: Vec<String>) -> AppResult<Self> {
        let mut gpg = UserGpgOptions::default();
        let mut paths = Vec::new();
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-d" => {
                    let homedir = args
                        .next()
                        .ok_or_else(|| "textconv option -d requires a gpg homedir".to_string())?;
                    gpg.homedir = Some(PathBuf::from(homedir));
                }
                "-p" => {
                    let passphrase = args
                        .next()
                        .ok_or_else(|| "textconv option -p requires a password".to_string())?;
                    gpg.passphrase = Some(passphrase);
                }
                _ if arg.starts_with('-') => {
                    return Err(format!("unknown textconv option '{}'", arg))
                }
                _ => paths.push(PathBuf::from(arg)),
            }
        }

        Ok(Self { gpg, paths })
    }
}

pub(crate) fn run(options: Options) -> AppResult<()> {
    if options.paths.is_empty() {
        return Err("textconv requires at least one encrypted file".to_string());
    }

    for path in options.paths {
        user_gpg(&options.gpg)
            .arg("--batch")
            .arg("--decrypt")
            .arg(&path)
            .status_ok(&format!("decrypt {}", path.display()))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn textconv_options_parse_homedir_password_and_paths() {
        let options = Options::parse(vec![
            "-d".to_string(),
            "keys".to_string(),
            "-p".to_string(),
            "secret".to_string(),
            "file.secret".to_string(),
        ])
        .unwrap();

        assert_eq!(options.gpg.homedir, Some(PathBuf::from("keys")));
        assert_eq!(options.gpg.passphrase, Some("secret".to_string()));
        assert_eq!(options.paths, vec![PathBuf::from("file.secret")]);
    }
}
