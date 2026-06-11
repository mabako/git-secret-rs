use std::fs;
use std::path::PathBuf;

use crate::git::{ensure_initialized, Repo};
use crate::mapping::Mapping;
use crate::paths::{encrypted_path, normalize_secret_path_for_repo};
use crate::AppResult;

#[derive(clap::Args)]
pub(crate) struct Options {
    #[arg(short = 'c')]
    clean_encrypted: bool,
    #[arg(short = 'h', long = "help")]
    help: bool,
    #[arg(value_name = "file")]
    paths: Vec<PathBuf>,
}

#[cfg(test)]
impl Options {
    pub(crate) fn parse(args: Vec<String>) -> AppResult<Self> {
        super::parse_options("git secret remove", args)
    }
}

pub(crate) fn run(options: Options) -> AppResult<()> {
    if options.help {
        print_help();
        return Ok(());
    }
    if options.paths.is_empty() {
        return Err("remove requires at least one file".to_string());
    }

    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;
    let mut mapping = Mapping::load(&repo)?;
    let mut removed = 0;

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
            println!("removed {}", normalized);
            removed += 1;
        }
    }

    if removed > 0 {
        mapping.save(&repo)?;
    }

    Ok(())
}

fn print_help() {
    println!(
        "git-secret-remove - stops files from being tracked by git-secret.\n\
\n\
Usage:\n\
  git secret remove [-c] [-h] <file> [file...]\n\
\n\
Options:\n\
  -c  deletes existing real encrypted files\n\
  -h  shows help"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_options_parse_clean_help_and_paths() {
        let options = Options::parse(vec![
            "-c".to_string(),
            "-h".to_string(),
            "secret.txt".to_string(),
        ])
        .unwrap();

        assert!(options.clean_encrypted);
        assert!(options.help);
        assert_eq!(options.paths, vec![PathBuf::from("secret.txt")]);
    }

    #[test]
    fn remove_options_reject_unknown_flags() {
        assert!(Options::parse(vec!["-v".to_string()]).is_err());
    }
}
