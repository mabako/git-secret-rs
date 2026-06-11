use std::path::PathBuf;

use crate::crypto::sha256_file;
use crate::git::{ensure_initialized, Repo};
use crate::mapping::Mapping;
use crate::paths::normalize_secret_path;
use crate::AppResult;

pub(crate) fn run(paths: Vec<PathBuf>) -> AppResult<()> {
    if paths.is_empty() {
        return Err("add requires at least one file".to_string());
    }

    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;
    let mut mapping = Mapping::load(&repo)?;
    let mut added = 0;

    for path in paths {
        let normalized = normalize_secret_path(&path)?;
        let plaintext = repo.join(&normalized);
        if !plaintext.is_file() {
            return Err(format!("{} is not a file", normalized));
        }
        let digest = sha256_file(&plaintext)?;
        if mapping.insert_or_update(normalized.clone(), digest) {
            println!("added {}", normalized);
            added += 1;
        }
    }

    if added > 0 {
        mapping.save(&repo)?;
    }

    Ok(())
}
