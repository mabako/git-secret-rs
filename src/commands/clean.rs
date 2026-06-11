use std::fs;
use std::path::PathBuf;

use crate::git::{ensure_initialized, Repo};
use crate::mapping::Mapping;
use crate::paths::selected_paths;
use crate::AppResult;

pub(crate) fn run(paths: Vec<PathBuf>) -> AppResult<()> {
    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;
    let mapping = Mapping::load(&repo)?;
    let paths = selected_paths(&repo, &mapping, paths)?;

    for path in paths {
        let plaintext = repo.join(&path);
        if plaintext.exists() {
            fs::remove_file(&plaintext)
                .map_err(|e| format!("remove {}: {}", plaintext.display(), e))?;
            println!("removed {}", path);
        }
    }

    Ok(())
}
