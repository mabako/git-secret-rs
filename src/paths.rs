use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::git::Repo;
use crate::mapping::Mapping;
use crate::AppResult;

pub(crate) fn selected_paths(mapping: &Mapping, paths: Vec<PathBuf>) -> AppResult<Vec<String>> {
    if paths.is_empty() {
        return Ok(mapping.paths());
    }

    paths
        .into_iter()
        .map(|path| normalize_secret_path(&path))
        .collect()
}

pub(crate) fn encrypted_path(repo: &Repo, path: &str) -> PathBuf {
    repo.join(format!("{}.secret", path))
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
    if normalized.ends_with(".secret") {
        return Err("add the plaintext path, not the .secret file".to_string());
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
    fn reject_parent_paths() {
        assert!(normalize_secret_path(Path::new("../secrets.env")).is_err());
        assert!(normalize_secret_path(Path::new("config/../secrets.env")).is_err());
    }

    #[test]
    fn reject_secret_suffix() {
        assert!(normalize_secret_path(Path::new("secrets.env.secret")).is_err());
    }
}
