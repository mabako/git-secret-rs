use crate::git::{ensure_initialized, recipient_records, Repo};
use crate::AppResult;

#[derive(clap::Args)]
pub(crate) struct Options {
    #[arg(short = 'l', help = "'long' output, shows key expiration dates.")]
    long: bool,
}

pub(crate) fn run(options: Options) -> AppResult<()> {
    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;
    let recipients = recipient_records(&repo)?;

    if recipients.is_empty() {
        println!("no recipients configured");
        return Ok(());
    }

    for recipient in recipients {
        if options.long {
            println!("{} (expires: {})", recipient.uid, recipient.expires);
        } else {
            println!("{}", recipient.uid);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Args, FromArgMatches};

    fn command() -> clap::Command {
        Options::augment_args(clap::Command::new("whoknows"))
    }

    #[test]
    fn whoknows_options_parse_long_and_help() {
        let matches = command().try_get_matches_from(["whoknows", "-l"]).unwrap();
        let options = Options::from_arg_matches(&matches).unwrap();
        assert!(options.long);
    }

    #[test]
    fn whoknows_options_reject_unknown_flags() {
        assert!(command()
            .try_get_matches_from(["whoknows", "--long"])
            .is_err());
    }
}
