use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::AppResult;

pub(crate) const SECRET_DIR: &str = ".gitsecret";
pub(crate) const KEYS_DIR: &str = ".gitsecret/keys";
pub(crate) const PATHS_DIR: &str = ".gitsecret/paths";
pub(crate) const MAPPING_FILE: &str = ".gitsecret/paths/mapping.cfg";
const MINGW64_GPG: &str = r"C:\Program Files (x86)\GnuPG\bin\gpg.exe";

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
}

pub(crate) fn ensure_initialized(repo: &Repo) -> AppResult<()> {
    if !repo.join(MAPPING_FILE).is_file() {
        return Err("repository is not initialized; run 'git secret init' first".to_string());
    }

    Ok(())
}

pub(crate) fn repo_gpg(repo: &Repo) -> Command {
    let mut command = gpg_command();
    command.arg("--homedir").arg(repo.join(KEYS_DIR));
    command
}

#[derive(Default)]
pub(crate) struct UserGpgOptions {
    pub(crate) homedir: Option<PathBuf>,
    pub(crate) passphrase: Option<String>,
}

pub(crate) fn user_gpg(options: &UserGpgOptions) -> Command {
    let mut command = gpg_command();
    command.arg("--quiet").arg("--no-tty");
    if let Some(homedir) = &options.homedir {
        command.arg("--homedir").arg(homedir);
    }
    if let Some(passphrase) = &options.passphrase {
        command
            .arg("--pinentry-mode")
            .arg("loopback")
            .arg("--passphrase")
            .arg(passphrase);
    }
    command
}

pub(crate) fn gpg_command() -> Command {
    Command::new(gpg_program_for_msystem(env::var("MSYSTEM").ok().as_deref()))
}

fn gpg_program_for_msystem(msystem: Option<&str>) -> &'static str {
    match msystem {
        Some("MINGW64") => MINGW64_GPG,
        _ => "gpg",
    }
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
    fn gpg_program_uses_original_gnupg_under_mingw64() {
        assert_eq!(
            gpg_program_for_msystem(Some("MINGW64")),
            r"C:\Program Files (x86)\GnuPG\bin\gpg.exe"
        );
    }

    #[test]
    fn gpg_program_uses_path_lookup_outside_mingw64() {
        assert_eq!(gpg_program_for_msystem(None), "gpg");
        assert_eq!(gpg_program_for_msystem(Some("MSYS")), "gpg");
    }
}
