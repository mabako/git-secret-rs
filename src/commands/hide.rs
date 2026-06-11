use std::fs;
use std::path::PathBuf;

use crate::crypto::sha256_file;
use crate::git::{ensure_initialized, recipient_key_ids, repo_gpg, Repo};
use crate::mapping::Mapping;
use crate::paths::{encrypted_path, selected_paths};
use crate::process::CommandExt;
use crate::AppResult;

pub(crate) struct Options {
    force: bool,
    delete_plaintext: bool,
    paths: Vec<PathBuf>,
}

impl Options {
    pub(crate) fn parse(args: Vec<String>) -> AppResult<Self> {
        let mut force = false;
        let mut delete_plaintext = false;
        let mut paths = Vec::new();

        for arg in args {
            match arg.as_str() {
                "-f" | "--force" => force = true,
                "-d" | "--delete" => delete_plaintext = true,
                _ if arg.starts_with('-') => return Err(format!("unknown hide option '{}'", arg)),
                _ => paths.push(PathBuf::from(arg)),
            }
        }

        Ok(Self {
            force,
            delete_plaintext,
            paths,
        })
    }
}

pub(crate) fn run(options: Options) -> AppResult<()> {
    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;
    let recipients = recipient_key_ids(&repo)?;
    if recipients.is_empty() {
        return Err("no recipients configured; run 'git secret tell <key>' first".to_string());
    }

    let mut mapping = Mapping::load(&repo)?;
    let paths = selected_paths(&mapping, options.paths)?;
    let mut mapping_changed = false;
    for path in paths {
        let input = repo.join(&path);
        let output = encrypted_path(&repo, &path);

        if !input.is_file() {
            return Err(format!("{} is not a file", path));
        }
        if output.exists() && !options.force {
            return Err(format!(
                "{} already exists; pass --force to overwrite",
                output.display()
            ));
        }

        let mut cmd = repo_gpg(&repo);
        cmd.arg("--batch")
            .arg("--yes")
            .arg("--trust-model")
            .arg("always")
            .arg("--encrypt");
        for recipient in &recipients {
            cmd.arg("--recipient").arg(recipient);
        }
        cmd.arg("--output").arg(&output).arg(&input);
        cmd.status_ok(&format!("encrypt {}", path))?;
        let digest = sha256_file(&input)?;
        if mapping.insert_or_update(path.clone(), digest) {
            mapping_changed = true;
        }

        if options.delete_plaintext {
            fs::remove_file(&input).map_err(|e| format!("remove {}: {}", input.display(), e))?;
        }

        println!("encrypted {}", path);
    }

    if mapping_changed {
        mapping.save(&repo)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hide_options_parse_flags_and_paths() {
        let options = Options::parse(vec![
            "--force".to_string(),
            "--delete".to_string(),
            "secret.env".to_string(),
        ])
        .unwrap();

        assert!(options.force);
        assert!(options.delete_plaintext);
        assert_eq!(options.paths, vec![PathBuf::from("secret.env")]);
    }
}
