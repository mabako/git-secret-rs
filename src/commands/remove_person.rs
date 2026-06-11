use crate::git::{ensure_initialized, repo_gpg, Repo};
use crate::process::CommandExt;
use crate::AppResult;

#[derive(clap::Args)]
pub(crate) struct Options {
    #[arg(value_name = "email-or-fingerprint")]
    keys: Vec<String>,
}

pub(crate) fn run(options: Options) -> AppResult<()> {
    if options.keys.is_empty() {
        return Err("removeperson requires at least one fingerprint, key id, or email".to_string());
    }

    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;

    for key in options.keys {
        repo_gpg(&repo)
            .arg("--batch")
            .arg("--yes")
            .arg("--delete-keys")
            .arg(&key)
            .status_ok(&format!("remove recipient {}", key))?;
        println!("removed recipient {}", key);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Args, FromArgMatches};

    fn command() -> clap::Command {
        Options::augment_args(clap::Command::new("removeperson"))
    }

    #[test]
    fn removeperson_options_parse_help_and_keys() {
        let matches = command()
            .try_get_matches_from(["removeperson", "user@example.com"])
            .unwrap();
        let options = Options::from_arg_matches(&matches).unwrap();
        assert_eq!(options.keys, vec!["user@example.com".to_string()]);
    }

    #[test]
    fn removeperson_options_reject_unknown_flags() {
        assert!(command()
            .try_get_matches_from(["removeperson", "-v"])
            .is_err());
    }
}
