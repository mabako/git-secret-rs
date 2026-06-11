use std::path::PathBuf;

use crate::git::{ensure_initialized, Repo};
use crate::mapping::Mapping;
use crate::paths::normalize_secret_path;
use crate::AppResult;

pub(crate) fn run(paths: Vec<PathBuf>) -> AppResult<()> {
    if paths.is_empty() {
        return Err("remove requires at least one file".to_string());
    }

    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;
    let mut mapping = Mapping::load(&repo)?;
    let mut removed = 0;

    for path in paths {
        let normalized = normalize_secret_path(&path)?;
        if mapping.remove(&normalized) {
            println!("removed {}", normalized);
            removed += 1;
        }
    }

    if removed > 0 {
        mapping.save(&repo)?;
    }

    Ok(())
}
