use crate::git::{ensure_initialized, gpg, Repo};
use crate::process::CommandExt;
use crate::AppResult;

pub(crate) fn run(keys: Vec<String>) -> AppResult<()> {
    if keys.is_empty() {
        return Err("removeperson requires at least one key id or email".to_string());
    }

    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;

    for key in keys {
        gpg(&repo)
            .arg("--batch")
            .arg("--yes")
            .arg("--delete-keys")
            .arg(&key)
            .status_ok(&format!("remove recipient {}", key))?;
        println!("removed recipient {}", key);
    }

    Ok(())
}
