use std::fs;

use crate::git::{mapping_file, Repo};
use crate::AppResult;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct MappingEntry {
    pub(crate) path: String,
    pub(crate) sha256: String,
}

pub(crate) struct Mapping {
    pub(crate) entries: Vec<MappingEntry>,
}

impl Mapping {
    pub(crate) fn load(repo: &Repo) -> AppResult<Self> {
        let path = repo.join(mapping_file());
        let content =
            fs::read_to_string(&path).map_err(|e| format!("read {}: {}", path.display(), e))?;

        let mut entries = Vec::new();
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let (path, sha256) = parse_mapping_line(trimmed)?;
            if !entries
                .iter()
                .any(|existing: &MappingEntry| existing.path == path)
            {
                entries.push(MappingEntry { path, sha256 });
            }
        }
        Ok(Self { entries })
    }

    pub(crate) fn insert_or_update(&mut self, path: String, sha256: String) -> bool {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.path == path) {
            if entry.sha256 == sha256 {
                return false;
            }

            entry.sha256 = sha256;
            return true;
        }

        self.entries.push(MappingEntry { path, sha256 });
        true
    }

    pub(crate) fn remove(&mut self, path: &str) -> bool {
        let old_len = self.entries.len();
        self.entries.retain(|entry| entry.path != path);
        old_len != self.entries.len()
    }

    pub(crate) fn paths(&self) -> Vec<String> {
        self.entries
            .iter()
            .map(|entry| entry.path.clone())
            .collect()
    }

    pub(crate) fn save(&self, repo: &Repo) -> AppResult<()> {
        let path = repo.join(mapping_file());
        let mut content = self
            .entries
            .iter()
            .map(|entry| format!("{}:{}", entry.path, entry.sha256))
            .collect::<Vec<_>>()
            .join("\n");
        if !content.is_empty() {
            content.push('\n');
        }

        fs::write(&path, content).map_err(|e| format!("write {}: {}", path.display(), e))
    }
}

pub(crate) fn parse_mapping_line(line: &str) -> AppResult<(String, String)> {
    if let Some((path, sha256)) = line.rsplit_once(':') {
        if path.is_empty() {
            return Err("mapping entry has an empty path".to_string());
        }
        if !sha256.is_empty() && !is_sha256_hex(sha256) {
            return Err(format!(
                "mapping entry for '{}' has an invalid sha256",
                path
            ));
        }

        Ok((path.to_string(), sha256.to_ascii_lowercase()))
    } else {
        Ok((line.to_string(), String::new()))
    }
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapping_preserves_order_and_unique_paths() {
        let mut mapping = Mapping {
            entries: Vec::new(),
        };
        assert!(mapping.insert_or_update("b.env".to_string(), "b".repeat(64)));
        assert!(mapping.insert_or_update("a.env".to_string(), "a".repeat(64)));
        assert!(mapping.insert_or_update("b.env".to_string(), "c".repeat(64)));
        assert!(!mapping.insert_or_update("a.env".to_string(), "a".repeat(64)));
        assert_eq!(
            mapping.entries,
            vec![
                MappingEntry {
                    path: "b.env".to_string(),
                    sha256: "c".repeat(64),
                },
                MappingEntry {
                    path: "a.env".to_string(),
                    sha256: "a".repeat(64),
                }
            ]
        );
    }

    #[test]
    fn mapping_entry_parses_sha256() {
        let digest = "81ade6f4f3c9f5d447f8b5b646da9ac9a2e6119cfde90504f156a8d93c8963a5";
        assert_eq!(
            parse_mapping_line(&format!("aaaa.txt:{}", digest)).unwrap(),
            ("aaaa.txt".to_string(), digest.to_string())
        );
    }

    #[test]
    fn mapping_entry_allows_legacy_path_only_lines() {
        assert_eq!(
            parse_mapping_line("legacy.txt").unwrap(),
            ("legacy.txt".to_string(), String::new())
        );
    }
}
