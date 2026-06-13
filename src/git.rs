use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::gpg::{gpg_arg_path, gpg_command};
use crate::AppResult;

const DEFAULT_SECRET_DIR: &str = ".gitsecret";
const KEYS_DIR_NAME: &str = "keys";
const PATHS_DIR_NAME: &str = "paths";
const MAPPING_FILE_NAME: &str = "mapping.cfg";
const SECRETS_DIR_ENV: &str = "SECRETS_DIR";

pub(crate) struct RecipientRecord {
    pub(crate) uid: String,
    pub(crate) expires: String,
}

pub(crate) struct Repo {
    root: PathBuf,
}

impl Repo {
    pub(crate) fn discover() -> AppResult<Self> {
        let output = Command::new("git")
            .arg("rev-parse")
            .arg("--show-toplevel")
            .output()
            .map_err(|e| format!("run git rev-parse: {}", e))?;

        if !output.status.success() {
            return Err("not inside a git repository".to_string());
        }

        let root = String::from_utf8(output.stdout)
            .map_err(|_| "git returned a non-UTF-8 repository path".to_string())?;
        Ok(Self {
            root: PathBuf::from(root.trim()),
        })
    }

    pub(crate) fn join<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        self.root.join(path)
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }
}

pub(crate) fn ensure_initialized(repo: &Repo) -> AppResult<()> {
    if !repo.join(mapping_file()).is_file() {
        return Err("repository is not initialized; run 'git secret init' first".to_string());
    }

    Ok(())
}

pub(crate) fn validate_repository_state() -> AppResult<Repo> {
    let repo = Repo::discover()?;
    ensure_secret_dir_is_not_ignored(&repo)?;
    ensure_repository_keyring_has_no_secret_keys(&repo)?;
    Ok(repo)
}

fn ensure_secret_dir_is_not_ignored(repo: &Repo) -> AppResult<()> {
    let gitignore = repo.join(".gitignore");
    let content = match fs::read_to_string(&gitignore) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("read {}: {}", gitignore.display(), error)),
    };
    let secret_dir = gitignore_path(&secret_dir());
    for line in content.lines() {
        if gitignore_pattern_ignores_secret_dir(line, &secret_dir) {
            return Err(format!(
                "secret directory {} must not be ignored by .gitignore",
                secret_dir
            ));
        }
    }

    Ok(())
}

fn gitignore_pattern_ignores_secret_dir(line: &str, secret_dir: &str) -> bool {
    let pattern = line.trim();
    if pattern.is_empty() || pattern.starts_with('#') || pattern.starts_with('!') {
        return false;
    }

    let pattern = pattern.trim_start_matches('/').trim_end_matches('/');
    let pattern = pattern
        .strip_suffix("/**")
        .or_else(|| pattern.strip_suffix("/*"))
        .unwrap_or(pattern);
    pattern == secret_dir
}

fn gitignore_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub(crate) fn ensure_repository_keyring_has_no_secret_keys(repo: &Repo) -> AppResult<()> {
    let legacy_secret_keyring = repo.join(keys_dir()).join("secring.gpg");
    if legacy_secret_keyring.is_file()
        && fs::metadata(&legacy_secret_keyring)
            .map(|metadata| metadata.len() > 0)
            .unwrap_or(false)
    {
        return Err("repository keyring contains secret keys".to_string());
    }

    Ok(())
}

pub(crate) fn ensure_valid_key_selector(key: &str) -> AppResult<()> {
    if looks_like_email(key) || looks_like_hex_key_id(key) {
        return Ok(());
    }

    Err(format!(
        "'{}' is not an email address, fingerprint, or key id",
        key
    ))
}

fn looks_like_email(key: &str) -> bool {
    let Some((local_part, domain)) = key.split_once('@') else {
        return false;
    };
    !local_part.is_empty() && !domain.is_empty()
}

fn looks_like_hex_key_id(key: &str) -> bool {
    key.len() >= 8 && key.chars().all(|character| character.is_ascii_hexdigit())
}

pub(crate) fn repo_gpg(repo: &Repo) -> Command {
    let mut command = gpg_command();
    command
        .arg("--homedir")
        .arg(gpg_arg_path(&repo.join(keys_dir())));
    command
}

pub(crate) fn secret_dir() -> PathBuf {
    env::var_os(SECRETS_DIR_ENV)
        .map(PathBuf::from)
        .filter(|path| is_valid_secret_dir(path))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_SECRET_DIR))
}

pub(crate) fn keys_dir() -> PathBuf {
    secret_dir().join(KEYS_DIR_NAME)
}

pub(crate) fn paths_dir() -> PathBuf {
    secret_dir().join(PATHS_DIR_NAME)
}

pub(crate) fn mapping_file() -> PathBuf {
    paths_dir().join(MAPPING_FILE_NAME)
}

fn is_valid_secret_dir(path: &Path) -> bool {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return false;
    }

    let mut has_component = false;
    for component in path.components() {
        use std::path::Component::*;
        match component {
            Normal(_) => has_component = true,
            CurDir | ParentDir | RootDir | Prefix(_) => return false,
        }
    }
    has_component
}

pub(crate) fn recipient_key_ids(repo: &Repo) -> AppResult<Vec<String>> {
    let output = repo_gpg(repo)
        .arg("--with-colons")
        .arg("--list-keys")
        .stdout(Stdio::piped())
        .output()
        .map_err(|e| format!("list recipients: {}", e))?;
    if !output.status.success() {
        return Err("could not list repository recipients".to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut recipients = Vec::new();
    for line in stdout.lines() {
        let fields: Vec<&str> = line.split(':').collect();
        if fields.first() == Some(&"pub") {
            if let Some(key_id) = fields.get(4) {
                if !key_id.is_empty() {
                    recipients.push((*key_id).to_string());
                }
            }
        }
    }

    Ok(recipients)
}

pub(crate) fn recipient_records(repo: &Repo) -> AppResult<Vec<RecipientRecord>> {
    let output = repo_gpg(repo)
        .arg("--with-colons")
        .arg("--list-keys")
        .stdout(Stdio::piped())
        .output()
        .map_err(|e| format!("list recipients: {}", e))?;
    if !output.status.success() {
        return Err("could not list repository recipients".to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut recipients = Vec::new();
    let mut current_expiration = "never".to_string();
    for line in stdout.lines() {
        let fields: Vec<&str> = line.split(':').collect();
        match fields.first().copied() {
            Some("pub") => {
                current_expiration = fields
                    .get(6)
                    .and_then(|expires| format_gpg_expiration(expires))
                    .unwrap_or_else(|| "never".to_string());
            }
            Some("uid") => {
                if let Some(uid) = fields.get(9) {
                    if !uid.is_empty() {
                        recipients.push(RecipientRecord {
                            uid: (*uid).to_string(),
                            expires: current_expiration.clone(),
                        });
                    }
                }
            }
            _ => {}
        }
    }

    Ok(recipients)
}

fn format_gpg_expiration(value: &str) -> Option<String> {
    if value.is_empty() {
        return None;
    }

    value
        .parse::<i64>()
        .ok()
        .map(|timestamp| format_unix_date(timestamp))
}

fn format_unix_date(timestamp: i64) -> String {
    let days = timestamp.div_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!("{:04}-{:02}-{:02}", year, month, day)
}

fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = year + if month <= 2 { 1 } else { 0 };

    (year as i32, month as u32, day as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_gpg_expiration_handles_empty_values() {
        assert_eq!(format_gpg_expiration(""), None);
    }

    #[test]
    fn format_gpg_expiration_formats_unix_dates() {
        assert_eq!(
            format_gpg_expiration("1453490413"),
            Some("2016-01-22".to_string())
        );
    }

    #[test]
    fn secret_dir_uses_valid_relative_paths() {
        assert!(is_valid_secret_dir(Path::new(".custom-secret")));
        assert!(is_valid_secret_dir(Path::new("secrets/store")));
        assert!(!is_valid_secret_dir(Path::new("")));
        assert!(!is_valid_secret_dir(Path::new(".")));
        assert!(!is_valid_secret_dir(Path::new("../secrets")));

        let absolute = std::env::current_dir().unwrap().join("secrets");
        assert!(!is_valid_secret_dir(&absolute));

        #[cfg(windows)]
        assert!(!is_valid_secret_dir(Path::new(r"C:\secrets")));
    }

    #[test]
    fn key_selector_accepts_email_and_hex_key_ids() {
        assert!(ensure_valid_key_selector("user@example.com").is_ok());
        assert!(ensure_valid_key_selector("D2805A4182E99FF4").is_ok());
        assert!(ensure_valid_key_selector("CE82DD3AFC167295F9132371D2805A4182E99FF4").is_ok());
        assert!(ensure_valid_key_selector("user").is_err());
    }
}
