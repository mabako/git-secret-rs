use crate::git::{ensure_initialized, Repo};
use crate::mapping::Mapping;
use crate::AppResult;

#[derive(clap::Args)]
pub(crate) struct Options {}

#[cfg(test)]
impl Options {
    pub(crate) fn parse(args: Vec<String>) -> AppResult<Self> {
        super::parse_options("git secret list", args)
    }
}

pub(crate) fn run(options: Options) -> AppResult<()> {
    let _ = options;

    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;

    for path in Mapping::load(&repo)?.paths() {
        println!("{}", path);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_options_parse_help() {
        assert!(Options::parse(vec!["-h".to_string()]).is_err());
    }

    #[test]
    fn list_options_reject_unknown_flags() {
        assert!(Options::parse(vec!["-v".to_string()]).is_err());
    }
}
