use std::fs;
use std::path::PathBuf;

use crate::git::{ensure_initialized, Repo};
use crate::mapping::Mapping;
use crate::paths::normalize_secret_path_for_repo;
use crate::AppResult;

#[derive(clap::Args)]
pub(crate) struct Options {
    #[arg(short = 'i', hide = true)]
    _ignore: bool,
    #[arg(short = 'v', help = "Print files as they are added")]
    verbose: bool,
    #[arg(value_name = "filename")]
    paths: Vec<PathBuf>,
}

pub(crate) fn run(options: Options) -> AppResult<()> {
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
            if options.verbose {
                println!("git-secret: adding file: {}", normalized);
            }
            added += 1;
        }
        add_to_gitignore(&repo, &normalized)?;
    }

    if added > 0 {
        mapping.save(&repo)?;
    }

    println!("git-secret: {} item(s) added.", added);
    Ok(())
}

fn add_to_gitignore(repo: &Repo, path: &str) -> AppResult<()> {
    let gitignore = repo.join(".gitignore");
    let pattern = gitignore_pattern_for_path(path);
    let content = match fs::read_to_string(&gitignore) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("read {}: {}", gitignore.display(), error)),
    };

    if content.lines().any(|line| line.trim() == pattern) {
        return Ok(());
    }

    let mut updated = content;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(&pattern);
    updated.push('\n');

    fs::write(&gitignore, updated).map_err(|e| format!("write {}: {}", gitignore.display(), e))
}

fn gitignore_pattern_for_path(path: &str) -> String {
    let mut pattern = String::new();

    for (index, character) in path.chars().enumerate() {
        if matches!(character, '*' | '?' | '[' | ']')
            || (index == 0 && matches!(character, '!' | '#'))
        {
            pattern.push('\\');
        }
        pattern.push(character);
    }

    pattern
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Args, FromArgMatches};

    fn command() -> clap::Command {
        Options::augment_args(clap::Command::new("add"))
    }

    #[test]
    fn add_options_parse_help_and_paths() {
        let matches = command()
            .try_get_matches_from(["add", "-i", "-v", "file.txt"])
            .unwrap();
        let options = Options::from_arg_matches(&matches).unwrap();

        assert!(options._ignore);
        assert!(options.verbose);
        assert_eq!(options.paths, vec![PathBuf::from("file.txt")]);
    }

    #[test]
    fn add_options_reject_unknown_flags() {
        assert!(command().try_get_matches_from(["add", "-d"]).is_err());
    }

    #[test]
    fn gitignore_pattern_escapes_glob_metacharacters() {
        assert_eq!(
            gitignore_pattern_for_path("space file three [] * $"),
            "space file three \\[\\] \\* $"
        );
    }
}
