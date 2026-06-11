use std::path::Path;

use crate::crypto::sha256_file;
use crate::git::{ensure_initialized, Repo};
use crate::mapping::Mapping;
use crate::paths::encrypted_path;
use crate::AppResult;

pub(crate) fn run() -> AppResult<()> {
    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;
    let mapping = Mapping::load(&repo)?;
    let mut changed = false;

    for entry in mapping.entries {
        let plaintext = repo.join(&entry.path);
        let secret = encrypted_path(&repo, &entry.path);
        let status = file_change_status(&plaintext, &secret, &entry.sha256)?;
        if let Some(status) = status {
            println!("{}\t{}", status, entry.path);
            changed = true;
        }
    }

    if !changed {
        println!("no changes");
    }

    Ok(())
}

fn file_change_status(
    plaintext: &Path,
    secret: &Path,
    stored_sha256: &str,
) -> AppResult<Option<&'static str>> {
    if !plaintext.exists() {
        return Ok(None);
    }
    if !secret.exists() {
        return Ok(Some("new"));
    }
    if stored_sha256.is_empty() || sha256_file(plaintext)? != stored_sha256 {
        Ok(Some("modified"))
    } else {
        Ok(None)
    }
}
