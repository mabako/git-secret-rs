use crate::git::{ensure_initialized, recipient_ids, Repo};
use crate::AppResult;

pub(crate) fn run() -> AppResult<()> {
    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;
    let recipients = recipient_ids(&repo)?;

    if recipients.is_empty() {
        println!("no recipients configured");
        return Ok(());
    }

    for recipient in recipients {
        println!("{}", recipient);
    }

    Ok(())
}
