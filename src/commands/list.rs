use crate::git::{ensure_initialized, Repo};
use crate::mapping::Mapping;
use crate::AppResult;

#[derive(clap::Args)]
pub(crate) struct Options {
    #[arg(short = 'h', long = "help")]
    help: bool,
}

#[cfg(test)]
impl Options {
    pub(crate) fn parse(args: Vec<String>) -> AppResult<Self> {
        super::parse_options("git secret list", args)
    }
}

pub(crate) fn run(options: Options) -> AppResult<()> {
    if options.help {
        print_help();
        return Ok(());
    }

    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;

    for path in Mapping::load(&repo)?.paths() {
        println!("{}", path);
    }

    Ok(())
}

fn print_help() {
    println!(
        "git secret list - print the files currently considered secret in this repo\n\
\n\
Usage:\n\
  git secret list [-h]\n\
\n\
Options:\n\
  -h  shows this help"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_options_parse_help() {
        let options = Options::parse(vec!["-h".to_string()]).unwrap();
        assert!(options.help);
    }

    #[test]
    fn list_options_reject_unknown_flags() {
        assert!(Options::parse(vec!["-v".to_string()]).is_err());
    }
}
