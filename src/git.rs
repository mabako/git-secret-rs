use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::AppResult;

pub(crate) const SECRET_DIR: &str = ".gitsecret";
pub(crate) const KEYS_DIR: &str = ".gitsecret/keys";
pub(crate) const PATHS_DIR: &str = ".gitsecret/paths";
pub(crate) const MAPPING_FILE: &str = ".gitsecret/paths/mapping.cfg";

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

pub(crate) fn gpg(repo: &Repo) -> Command {
    let mut command = Command::new("gpg");
    command.arg("--homedir").arg(repo.join(KEYS_DIR));
    command
}

pub(crate) fn recipient_key_ids(repo: &Repo) -> AppResult<Vec<String>> {
    let output = gpg(repo)
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

pub(crate) fn recipient_ids(repo: &Repo) -> AppResult<Vec<String>> {
    let output = gpg(repo)
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
        if fields.first() == Some(&"uid") {
            if let Some(uid) = fields.get(9) {
                if !uid.is_empty() {
                    recipients.push((*uid).to_string());
                }
            }
        }
    }

    Ok(recipients)
}
