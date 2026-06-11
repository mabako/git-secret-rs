use std::path::PathBuf;

use crate::git::{user_gpg, UserGpgOptions};
use crate::process::CommandExt;
use crate::AppResult;

#[derive(clap::Args)]
pub(crate) struct Options {
    #[command(flatten)]
    gpg: UserGpgOptions,
    #[arg(value_name = "encrypted-file")]
    paths: Vec<PathBuf>,
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
    use clap::{Args, FromArgMatches};

    fn command() -> clap::Command {
        Options::augment_args(clap::Command::new("textconv"))
    }

    #[test]
    fn textconv_options_parse_homedir_password_and_paths() {
        let matches = command()
            .try_get_matches_from(["textconv", "-d", "keys", "-p", "secret", "file.secret"])
            .unwrap();
        let options = Options::from_arg_matches(&matches).unwrap();

        assert_eq!(options.gpg.homedir, Some(PathBuf::from("keys")));
        assert_eq!(options.gpg.passphrase, Some("secret".to_string()));
        assert_eq!(options.paths, vec![PathBuf::from("file.secret")]);
    }
}
