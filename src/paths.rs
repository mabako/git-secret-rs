use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::git::Repo;
use crate::mapping::Mapping;
use crate::AppResult;

const DEFAULT_SECRET_EXTENSION: &str = ".secret";
const SECRETS_EXTENSION_ENV: &str = "SECRETS_EXTENSION";

pub(crate) fn selected_paths(
    repo: &Repo,
    mapping: &Mapping,
    paths: Vec<PathBuf>,
) -> AppResult<Vec<String>> {
    if paths.is_empty() {
        return Ok(mapping.paths());
    }

    paths
        .into_iter()
        .map(|path| normalize_secret_path_for_repo(repo, &path))
        .collect()
}

pub(crate) fn encrypted_path(repo: &Repo, path: &str) -> PathBuf {
    repo.join(format!("{}{}", path, secret_extension()))
}

pub(crate) fn secret_extension() -> String {
    std::env::var(SECRETS_EXTENSION_ENV)
        .ok()
        .filter(|extension| is_valid_secret_extension(extension))
        .unwrap_or_else(|| DEFAULT_SECRET_EXTENSION.to_string())
}

fn is_valid_secret_extension(extension: &str) -> bool {
    extension.starts_with('.')
        && extension.len() > 1
        && !extension.contains('/')
        && !extension.contains('\\')
}

pub(crate) fn normalize_secret_path_for_repo(repo: &Repo, path: &Path) -> AppResult<String> {
    let current_dir =
        std::env::current_dir().map_err(|e| format!("get current directory: {}", e))?;
    normalize_secret_path_from_current_dir(repo.root(), &current_dir, path)
}

fn normalize_secret_path_from_current_dir(
    repo_root: &Path,
    current_dir: &Path,
    path: &Path,
) -> AppResult<String> {
    if path.is_absolute() {
        return normalize_secret_path(path);
    }

    let repo_root = canonicalize_existing(repo_root)
        .map_err(|e| format!("resolve repository root {}: {}", repo_root.display(), e))?;
    let current_dir = canonicalize_existing(current_dir)
        .map_err(|e| format!("resolve current directory {}: {}", current_dir.display(), e))?;
    let current_relative = current_dir.strip_prefix(&repo_root).map_err(|_| {
        format!(
            "current directory {} is not inside repository {}",
            current_dir.display(),
            repo_root.display()
        )
    })?;
    let adjusted = if current_relative.as_os_str().is_empty() {
        path.to_path_buf()
    } else {
        current_relative.join(path)
    };

    normalize_secret_path(&adjusted)
}

fn canonicalize_existing(path: &Path) -> io::Result<PathBuf> {
    fs::canonicalize(path)
}

pub(crate) fn normalize_secret_path(path: &Path) -> AppResult<String> {
    if path.is_absolute() {
        return Err(format!(
            "{} must be relative to the repository",
            path.display()
        ));
    }

    let mut pieces = Vec::new();
    for component in path.components() {
        use std::path::Component::*;
        match component {
            CurDir => {}
            Normal(piece) => pieces.push(os_to_string(piece)?),
            ParentDir => return Err(format!("{} must not contain '..'", path.display())),
            RootDir | Prefix(_) => {
                return Err(format!(
                    "{} must be relative to the repository",
                    path.display()
                ))
            }
        }
    }

    if pieces.is_empty() {
        return Err("empty file path".to_string());
    }

    let normalized = pieces.join("/");
    if normalized.ends_with(&secret_extension()) {
        return Err("add the plaintext path, not the encrypted file".to_string());
    }

    Ok(normalized)
}

fn os_to_string(value: &OsStr) -> AppResult<String> {
    value
        .to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "path is not valid UTF-8".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_plain_relative_paths() {
        assert_eq!(
            normalize_secret_path(Path::new("./config/secrets.env")).unwrap(),
            "config/secrets.env"
        );
    }

    #[test]
    fn normalize_paths_relative_to_current_subdirectory() {
        let root = std::env::current_dir().unwrap();
        let subdir = root.join("src");

        assert_eq!(
            normalize_secret_path_from_current_dir(&root, &subdir, Path::new("main.rs")).unwrap(),
            "src/main.rs"
        );
    }

    #[test]
    fn reject_parent_paths() {
        assert!(normalize_secret_path(Path::new("../secrets.env")).is_err());
        assert!(normalize_secret_path(Path::new("config/../secrets.env")).is_err());
    }

    #[test]
    fn reject_secret_suffix() {
        assert!(normalize_secret_path(Path::new("secrets.env.secret")).is_err());
    }
}
