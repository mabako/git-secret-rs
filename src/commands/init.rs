use std::fs;

use crate::git::{repo_gpg, Repo, KEYS_DIR, MAPPING_FILE, PATHS_DIR, SECRET_DIR};
use crate::process::CommandExt;
use crate::AppResult;

const ROOT_GITIGNORE_ENTRIES: &[&str] = &[
    ".gitsecret/keys/random_seed",
    ".gitsecret/keys/*.lock",
    "!*.secret",
];

pub(crate) fn run() -> AppResult<()> {
    let repo = Repo::discover()?;
    fs::create_dir_all(repo.join(KEYS_DIR)).map_err(|e| format!("create {}: {}", KEYS_DIR, e))?;
    fs::create_dir_all(repo.join(PATHS_DIR)).map_err(|e| format!("create {}: {}", PATHS_DIR, e))?;

    let mapping = repo.join(MAPPING_FILE);
    if !mapping.exists() {
        fs::write(&mapping, "").map_err(|e| format!("write {}: {}", mapping.display(), e))?;
    }

    add_lines_to_root_gitignore(&repo, ROOT_GITIGNORE_ENTRIES)?;

    repo_gpg(&repo)
        .arg("--list-keys")
        .status_ok("initialize repository keyring")?;

    println!("created {}", repo.join(SECRET_DIR).display());
    Ok(())
}

fn add_lines_to_root_gitignore(repo: &Repo, entries: &[&str]) -> AppResult<()> {
    let gitignore = repo.join(".gitignore");
    let content = match fs::read_to_string(&gitignore) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("read {}: {}", gitignore.display(), error)),
    };

    let mut updated = content;
    let mut changed = false;
    for entry in entries {
        if updated.lines().any(|line| line.trim() == *entry) {
            continue;
        }
        if !updated.is_empty() && !updated.ends_with('\n') {
            updated.push('\n');
        }
        updated.push_str(entry);
        updated.push('\n');
        changed = true;
    }

    if changed {
        fs::write(&gitignore, updated)
            .map_err(|e| format!("write {}: {}", gitignore.display(), e))?;
    }

    Ok(())
}
