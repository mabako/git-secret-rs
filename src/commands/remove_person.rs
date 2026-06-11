use crate::git::{ensure_initialized, repo_gpg, Repo};
use crate::process::CommandExt;
use crate::AppResult;

#[derive(clap::Args)]
pub(crate) struct Options {
    #[arg(value_name = "fingerprint-or-key-id-or-email")]
    keys: Vec<String>,
    #[arg(short = 'h', long = "help")]
    help: bool,
}

#[cfg(test)]
impl Options {
    pub(crate) fn parse(args: Vec<String>) -> AppResult<Self> {
        super::parse_options("git secret removeperson", args)
    }
}

pub(crate) fn run(options: Options) -> AppResult<()> {
    if options.help {
        print_help();
        return Ok(());
    }
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

fn print_help() {
    println!(
        "git-secret-removeperson - removes public keys for passed email addresses or GPG fingerprints from repo's git-secret keyring.\n\
\n\
Usage:\n\
  git secret removeperson [-h] <fingerprint-or-key-id-or-email>...\n\
\n\
Options:\n\
  -h  shows this help"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removeperson_options_parse_help_and_keys() {
        let options =
            Options::parse(vec!["-h".to_string(), "user@example.com".to_string()]).unwrap();
        assert!(options.help);
        assert_eq!(options.keys, vec!["user@example.com".to_string()]);
    }

    #[test]
    fn removeperson_options_reject_unknown_flags() {
        assert!(Options::parse(vec!["-v".to_string()]).is_err());
    }
}
