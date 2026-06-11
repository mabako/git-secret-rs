use std::path::PathBuf;

use crate::git::{ensure_initialized, user_gpg_with_configured_passphrase, Repo};
use crate::paths::{encrypted_path, normalize_secret_path};
use crate::process::CommandExt;
use crate::AppResult;

pub(crate) fn run(paths: Vec<PathBuf>) -> AppResult<()> {
    if paths.is_empty() {
        return Err("cat requires at least one file".to_string());
    }

    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;

    for path in paths {
        let normalized = normalize_secret_path(&path)?;
        let secret = encrypted_path(&repo, &normalized);
        user_gpg_with_configured_passphrase()
            .arg("--batch")
            .arg("--decrypt")
            .arg(&secret)
            .status_ok(&format!("decrypt {}", normalized))?;
    }

    Ok(())
}
