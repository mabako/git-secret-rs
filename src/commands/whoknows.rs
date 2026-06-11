use crate::git::{ensure_initialized, recipient_records, Repo};
use crate::AppResult;

#[derive(clap::Args)]
pub(crate) struct Options {
    #[arg(short = 'l')]
    long: bool,
    #[arg(short = 'h', long = "help")]
    help: bool,
}

#[cfg(test)]
impl Options {
    pub(crate) fn parse(args: Vec<String>) -> AppResult<Self> {
        super::parse_options("git secret whoknows", args)
    }
}

pub(crate) fn run(options: Options) -> AppResult<()> {
    if options.help {
        print_help();
        return Ok(());
    }

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

fn print_help() {
    println!(
        "git-secret-whoknows - print email addresses allowed to access the secrets in this repo.\n\
\n\
Usage:\n\
  git secret whoknows [-l|-h]\n\
\n\
Options:\n\
  -l  long output, shows key expiration dates\n\
  -h  shows this help"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whoknows_options_parse_long_and_help() {
        let options = Options::parse(vec!["-l".to_string(), "-h".to_string()]).unwrap();
        assert!(options.long);
        assert!(options.help);
    }

    #[test]
    fn whoknows_options_reject_unknown_flags() {
        assert!(Options::parse(vec!["--long".to_string()]).is_err());
    }
}
