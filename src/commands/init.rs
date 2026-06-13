use std::fs;

use crate::git::{keys_dir, mapping_file, paths_dir, repo_gpg, secret_dir, Repo};
use crate::paths::secret_extension;
use crate::process::CommandExt;
use crate::AppResult;

#[derive(clap::Args)]
pub(crate) struct Options {}

pub(crate) fn run(options: Options) -> AppResult<()> {
    let _ = options;

    let repo = Repo::discover()?;
    let secret_dir = secret_dir();
    let keys_dir = keys_dir();
    let paths_dir = paths_dir();
    let keyring = repo.join(&keys_dir);
    if repo.join(&secret_dir).exists() && !repo.join(mapping_file()).is_file() {
        return Err("abort: already initialized.".to_string());
    }

    fs::create_dir_all(&keyring).map_err(|e| format!("create {}: {}", keys_dir.display(), e))?;
    restrict_keyring_permissions(&keyring)?;
    fs::create_dir_all(repo.join(&paths_dir))
        .map_err(|e| format!("create {}: {}", paths_dir.display(), e))?;

    let mapping = repo.join(mapping_file());
    if !mapping.exists() {
        fs::write(&mapping, "").map_err(|e| format!("write {}: {}", mapping.display(), e))?;
    }

    let extension = secret_extension();
    let gitignore_entries = root_gitignore_entries(&secret_dir, &extension);
    let gitattributes_entries = root_gitattributes_entries(&extension);
    add_lines_to_root_gitignore(&repo, &gitignore_entries)?;
    add_lines_to_root_file(&repo, ".gitattributes", &gitattributes_entries)?;
    configure_diff_driver()?;

    repo_gpg(&repo)
        .arg("--list-keys")
        .status_ok("initialize repository keyring")?;

    println!("created {}", repo.join(&secret_dir).display());
    Ok(())
}

fn root_gitignore_entries(secret_dir: &std::path::Path, extension: &str) -> Vec<String> {
    let secret_dir = secret_dir.to_string_lossy().replace('\\', "/");
    vec![
        format!("{}/keys/random_seed", secret_dir),
        format!("{}/keys/*.lock", secret_dir),
        format!("!*{}", extension),
    ]
}

fn root_gitattributes_entries(extension: &str) -> Vec<String> {
    vec![format!("*{} diff=git-secret", extension)]
}

fn add_lines_to_root_gitignore(repo: &Repo, entries: &[String]) -> AppResult<()> {
    add_lines_to_root_file(repo, ".gitignore", entries)
}

fn add_lines_to_root_file(repo: &Repo, file: &str, entries: &[String]) -> AppResult<()> {
    let path = repo.join(file);
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("read {}: {}", path.display(), error)),
    };

    let mut updated = content;
    let mut changed = false;
    for entry in entries {
        if updated.lines().any(|line| line.trim() == entry.as_str()) {
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

#[cfg(unix)]
fn restrict_keyring_permissions(path: &std::path::Path) -> AppResult<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|e| format!("set permissions on {}: {}", path.display(), e))
}

#[cfg(not(unix))]
fn restrict_keyring_permissions(_path: &std::path::Path) -> AppResult<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Args;

    fn command() -> clap::Command {
        Options::augment_args(clap::Command::new("init"))
    }

    #[test]
    fn init_options_parse_help() {
        assert!(command().try_get_matches_from(["init", "-h"]).is_err());
    }

    #[test]
    fn init_options_reject_unknown_flags() {
        assert!(command().try_get_matches_from(["init", "-v"]).is_err());
    }
}
