use std::io::Write;
use std::process::{Command, Stdio};

use crate::git::{ensure_initialized, gpg, Repo};
use crate::AppResult;

pub(crate) fn run(keys: Vec<String>) -> AppResult<Vec<String>> {
    if keys.is_empty() {
        return Err("tell requires at least one key id or email".to_string());
    }

    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;
    let mut imported = Vec::new();

    for key in keys {
        let exported = Command::new("gpg")
            .arg("--armor")
            .arg("--export")
            .arg(&key)
            .output()
            .map_err(|e| format!("run gpg --export {}: {}", key, e))?;
        if !exported.status.success() || exported.stdout.is_empty() {
            return Err(format!("could not export public key '{}'", key));
        }

        let mut child = gpg(&repo)
            .arg("--import")
            .stdin(Stdio::piped())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("run gpg --import: {}", e))?;

        child
            .stdin
            .as_mut()
            .ok_or_else(|| "could not open gpg import stdin".to_string())?
            .write_all(&exported.stdout)
            .map_err(|e| format!("write key to gpg import: {}", e))?;

        let status = child
            .wait()
            .map_err(|e| format!("wait for gpg import: {}", e))?;
        if !status.success() {
            return Err(format!("could not import public key '{}'", key));
        }

        println!("added recipient {}", key);
        imported.push(key);
    }

    Ok(imported)
}
