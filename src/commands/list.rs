use crate::git::{ensure_initialized, Repo};
use crate::mapping::Mapping;
use crate::AppResult;

pub(crate) fn run() -> AppResult<()> {
    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;

    for path in Mapping::load(&repo)?.paths() {
        println!("{}", path);
    }

    Ok(())
}
