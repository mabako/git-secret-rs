use crate::git::{ensure_initialized, recipient_records, Repo};
use crate::AppResult;

#[derive(clap::Args)]
pub(crate) struct Options {
    #[arg(short = 'l', help = "'long' output, shows key expiration dates.")]
    long: bool,
}

#[cfg(test)]
impl Options {
    pub(crate) fn parse(args: Vec<String>) -> AppResult<Self> {
        super::parse_options("git secret whoknows", args)
    }
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

    #[test]
    fn whoknows_options_parse_long_and_help() {
        let options = Options::parse(vec!["-l".to_string()]).unwrap();
        assert!(options.long);
    }

    #[test]
    fn whoknows_options_reject_unknown_flags() {
        assert!(Options::parse(vec!["--long".to_string()]).is_err());
    }
}
