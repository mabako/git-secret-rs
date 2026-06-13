use std::fs;
use std::path::PathBuf;

use crate::git::{ensure_initialized, Repo};
use crate::mapping::Mapping;
use crate::paths::{encrypted_path, normalize_secret_path_for_repo};
use crate::AppResult;

#[derive(clap::Args)]
pub(crate) struct Options {
    #[arg(short = 'c', help = "Deletes existing real encrypted files")]
    clean_encrypted: bool,
    #[arg(value_name = "filename")]
    paths: Vec<PathBuf>,
}

pub(crate) fn run(options: Options) -> AppResult<()> {
    if options.paths.is_empty() {
        return Err("remove requires at least one file".to_string());
    }

    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;
    let mut mapping = Mapping::load(&repo)?;
    let mut removed = Vec::new();

    for path in options.paths {
        let normalized = normalize_secret_path_for_repo(&repo, &path)?;
        if options.clean_encrypted {
            let encrypted = encrypted_path(&repo, &normalized);
            if encrypted.exists() {
                fs::remove_file(&encrypted)
                    .map_err(|e| format!("remove {}: {}", encrypted.display(), e))?;
            }
        }
        if mapping.remove(&normalized) {
            removed.push(normalized);
        }
    }

    if !removed.is_empty() {
        mapping.save(&repo)?;
        println!("git-secret: removed from index.");
        println!(
            "git-secret: ensure that files: [{}] are now not ignored.",
            removed.join(" ")
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Args, FromArgMatches};

    fn command() -> clap::Command {
        Options::augment_args(clap::Command::new("remove"))
    }

    #[test]
    fn remove_options_parse_clean_help_and_paths() {
        let matches = command()
            .try_get_matches_from(["remove", "-c", "secret.txt"])
            .unwrap();
        let options = Options::from_arg_matches(&matches).unwrap();

        assert!(options.clean_encrypted);
        assert_eq!(options.paths, vec![PathBuf::from("secret.txt")]);
    }

    #[test]
    fn remove_options_reject_unknown_flags() {
        assert!(command().try_get_matches_from(["remove", "-v"]).is_err());
    }
}
