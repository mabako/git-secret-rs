use std::fs;
use std::path::PathBuf;

use crate::git::{ensure_initialized, Repo};
use crate::mapping::Mapping;
use crate::paths::normalize_secret_path_for_repo;
use crate::AppResult;

#[derive(clap::Args)]
pub(crate) struct Options {
    #[arg(value_name = "file")]
    paths: Vec<PathBuf>,
    #[arg(short = 'h', long = "help")]
    help: bool,
}

#[cfg(test)]
impl Options {
    pub(crate) fn parse(args: Vec<String>) -> AppResult<Self> {
        super::parse_options("git secret add", args)
    }
}

pub(crate) fn run(options: Options) -> AppResult<()> {
    if options.help {
        print_help();
        return Ok(());
    }
    if options.paths.is_empty() {
        return Err("add requires at least one file".to_string());
    }

    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;
    let mut mapping = Mapping::load(&repo)?;
    let mut added = 0;

    for path in options.paths {
        let normalized = normalize_secret_path_for_repo(&repo, &path)?;
        let plaintext = repo.join(&normalized);
        if !plaintext.is_file() {
            return Err(format!("{} is not a file", normalized));
        }
        if mapping.insert_or_update(normalized.clone(), String::new()) {
            println!("added {}", normalized);
            added += 1;
        }
        add_to_gitignore(&repo, &normalized)?;
    }

    if added > 0 {
        mapping.save(&repo)?;
    }

    Ok(())
}

fn add_to_gitignore(repo: &Repo, path: &str) -> AppResult<()> {
    let gitignore = repo.join(".gitignore");
    let content = match fs::read_to_string(&gitignore) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("read {}: {}", gitignore.display(), error)),
    };

    if content.lines().any(|line| line.trim() == path) {
        return Ok(());
    }

    let mut updated = content;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(path);
    updated.push('\n');

    fs::write(&gitignore, updated).map_err(|e| format!("write {}: {}", gitignore.display(), e))
}

fn print_help() {
    println!(
        "git secret add - tells git secret which files hold secrets.\n\
\n\
Usage:\n\
  git secret add [-h] <file> [file...]\n\
\n\
Options:\n\
  -h  shows this help"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_options_parse_help_and_paths() {
        let options = Options::parse(vec!["-h".to_string(), "file.txt".to_string()]).unwrap();
        assert!(options.help);
        assert_eq!(options.paths, vec![PathBuf::from("file.txt")]);
    }

    #[test]
    fn add_options_reject_unknown_flags() {
        assert!(Options::parse(vec!["-d".to_string()]).is_err());
    }
}
