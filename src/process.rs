use std::process::Command;

use crate::AppResult;

pub(crate) trait CommandExt {
    fn status_ok(&mut self, action: &str) -> AppResult<()>;
}

impl CommandExt for Command {
    fn status_ok(&mut self, action: &str) -> AppResult<()> {
        let status = self
            .status()
            .map_err(|e| format!("{}: failed to run command: {}", action, e))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("{}: command exited with {}", action, status))
        }
    }
}
