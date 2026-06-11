use std::fs;

use crate::git::{repo_gpg, Repo, KEYS_DIR, MAPPING_FILE, PATHS_DIR, SECRET_DIR};
use crate::process::CommandExt;
use crate::AppResult;

const ROOT_GITIGNORE_ENTRIES: &[&str] = &[
    ".gitsecret/keys/random_seed",
    ".gitsecret/keys/*.lock",
    "!*.secret",
];
const ROOT_GITATTRIBUTES_ENTRIES: &[&str] = &["*.secret diff=git-secret"];

#[derive(clap::Args)]
pub(crate) struct Options {}

#[cfg(test)]
impl Options {
    pub(crate) fn parse(args: Vec<String>) -> AppResult<Self> {
        super::parse_options("git secret init", args)
    }
}

pub(crate) fn run(options: Options) -> AppResult<()> {
    let _ = options;

    let repo = Repo::discover()?;
    fs::create_dir_all(repo.join(KEYS_DIR)).map_err(|e| format!("create {}: {}", KEYS_DIR, e))?;
    fs::create_dir_all(repo.join(PATHS_DIR)).map_err(|e| format!("create {}: {}", PATHS_DIR, e))?;

    let mapping = repo.join(MAPPING_FILE);
    if !mapping.exists() {
        fs::write(&mapping, "").map_err(|e| format!("write {}: {}", mapping.display(), e))?;
    }

    add_lines_to_root_gitignore(&repo, ROOT_GITIGNORE_ENTRIES)?;
    add_lines_to_root_file(&repo, ".gitattributes", ROOT_GITATTRIBUTES_ENTRIES)?;
    configure_diff_driver()?;

    repo_gpg(&repo)
        .arg("--list-keys")
        .status_ok("initialize repository keyring")?;

    println!("created {}", repo.join(SECRET_DIR).display());
    Ok(())
}

fn add_lines_to_root_gitignore(repo: &Repo, entries: &[&str]) -> AppResult<()> {
    add_lines_to_root_file(repo, ".gitignore", entries)
}

fn add_lines_to_root_file(repo: &Repo, file: &str, entries: &[&str]) -> AppResult<()> {
    let path = repo.join(file);
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("read {}: {}", path.display(), error)),
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
        fs::write(&path, updated).map_err(|e| format!("write {}: {}", path.display(), e))?;
    }

    Ok(())
}

fn configure_diff_driver() -> AppResult<()> {
    std::process::Command::new("git")
        .arg("config")
        .arg("diff.git-secret.textconv")
        .arg("git-secret textconv")
        .status_ok("configure git-secret diff textconv")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_options_parse_help() {
        assert!(Options::parse(vec!["-h".to_string()]).is_err());
    }

    #[test]
    fn init_options_reject_unknown_flags() {
        assert!(Options::parse(vec!["-v".to_string()]).is_err());
    }
}
