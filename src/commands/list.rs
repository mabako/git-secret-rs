use crate::git::{ensure_initialized, Repo};
use crate::mapping::Mapping;
use crate::AppResult;

#[derive(clap::Args)]
pub(crate) struct Options {}

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
    use clap::Args;

    fn command() -> clap::Command {
        Options::augment_args(clap::Command::new("list"))
    }

    #[test]
    fn list_options_parse_help() {
        assert!(command().try_get_matches_from(["list", "-h"]).is_err());
    }

    #[test]
    fn list_options_reject_unknown_flags() {
        assert!(command().try_get_matches_from(["list", "-v"]).is_err());
    }
}
