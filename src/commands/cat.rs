use std::path::PathBuf;

use crate::git::{ensure_initialized, gpg_arg_path, user_gpg, Repo, UserGpgOptions};
use crate::paths::{encrypted_path, normalize_secret_path_for_repo};
use crate::process::CommandExt;
use crate::AppResult;

#[derive(clap::Args)]
pub(crate) struct Options {
    #[command(flatten)]
    gpg: UserGpgOptions,
    #[arg(value_name = "filename")]
    paths: Vec<PathBuf>,
}

pub(crate) fn run(options: Options) -> AppResult<()> {
    if options.paths.is_empty() {
        return Err("cat requires at least one file".to_string());
    }

    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;

    for path in options.paths {
        let normalized = normalize_secret_path_for_repo(&repo, &path)?;
        let secret = encrypted_path(&repo, &normalized);
        user_gpg(&options.gpg)
            .arg("--batch")
            .arg("--decrypt")
            .arg(gpg_arg_path(&secret))
            .status_ok(&format!("decrypt {}", normalized))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Args, FromArgMatches};

    fn command() -> clap::Command {
        Options::augment_args(clap::Command::new("cat"))
    }

    #[test]
    fn cat_options_parse_homedir_password_and_paths() {
        let matches = command()
            .try_get_matches_from(["cat", "-d", "keys", "-p", "secret", "file.txt"])
            .unwrap();
        let options = Options::from_arg_matches(&matches).unwrap();

        assert_eq!(options.gpg.homedir, Some(PathBuf::from("keys")));
        assert_eq!(options.gpg.passphrase, Some("secret".to_string()));
        assert_eq!(options.paths, vec![PathBuf::from("file.txt")]);
    }

    #[test]
    fn cat_options_parse_help() {
        assert!(command().try_get_matches_from(["cat", "-h"]).is_err());
    }
}
