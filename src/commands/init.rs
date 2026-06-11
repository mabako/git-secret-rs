use std::fs;

use crate::git::{repo_gpg, Repo, KEYS_DIR, MAPPING_FILE, PATHS_DIR, SECRET_DIR};
use crate::process::CommandExt;
use crate::AppResult;

pub(crate) fn run() -> AppResult<()> {
    let repo = Repo::discover()?;
    fs::create_dir_all(repo.join(KEYS_DIR)).map_err(|e| format!("create {}: {}", KEYS_DIR, e))?;
    fs::create_dir_all(repo.join(PATHS_DIR)).map_err(|e| format!("create {}: {}", PATHS_DIR, e))?;

    let mapping = repo.join(MAPPING_FILE);
    if !mapping.exists() {
        fs::write(&mapping, "").map_err(|e| format!("write {}: {}", mapping.display(), e))?;
    }

    let key_gitignore = repo.join(KEYS_DIR).join(".gitignore");
    if !key_gitignore.exists() {
        fs::write(
            &key_gitignore,
            "random_seed\ntrustdb.gpg\nS.gpg-agent*\nprivate-keys-v1.d/\n",
        )
        .map_err(|e| format!("write {}: {}", key_gitignore.display(), e))?;
    }

    repo_gpg(&repo)
        .arg("--list-keys")
        .status_ok("initialize repository keyring")?;

    println!("created {}", repo.join(SECRET_DIR).display());
    Ok(())
}
