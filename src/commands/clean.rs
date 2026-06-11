use std::fs;
use std::path::{Path, PathBuf};

use crate::git::{ensure_initialized, Repo};
use crate::paths::secret_extension;
use crate::AppResult;

#[derive(clap::Args)]
pub(crate) struct Options {
    #[arg(short = 'v', help = "verbose mode, shows which files are deleted.")]
    verbose: bool,
}

pub(crate) fn run(options: Options) -> AppResult<()> {
    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;
    let secret_files = secret_files(repo.root(), &secret_extension())?;
    let verbose = options.verbose || super::secrets_verbose();

    for path in secret_files {
        fs::remove_file(&path).map_err(|e| format!("remove {}: {}", path.display(), e))?;
        if verbose {
            println!("removed {}", repo_relative_path(repo.root(), &path)?);
        }
    }

    Ok(())
}

fn secret_files(root: &Path, extension: &str) -> AppResult<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_secret_files(root, extension, &mut files)?;
    Ok(files)
}

fn collect_secret_files(path: &Path, extension: &str, files: &mut Vec<PathBuf>) -> AppResult<()> {
    for entry in fs::read_dir(path).map_err(|e| format!("read {}: {}", path.display(), e))? {
        let entry = entry.map_err(|e| format!("read {} entry: {}", path.display(), e))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|e| format!("read {} type: {}", path.display(), e))?;
        if file_type.is_dir() {
            if entry.file_name() != ".git" {
                collect_secret_files(&path, extension, files)?;
            }
        } else if file_type.is_file()
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(extension))
        {
            files.push(path);
        }
    }

    Ok(())
}

fn repo_relative_path(repo_root: &Path, path: &Path) -> AppResult<String> {
    let relative = path
        .strip_prefix(repo_root)
        .map_err(|_| format!("{} is not inside {}", path.display(), repo_root.display()))?;
    let pieces = relative
        .components()
        .map(|component| {
            component
                .as_os_str()
                .to_str()
                .ok_or_else(|| "path is not valid UTF-8".to_string())
        })
        .collect::<AppResult<Vec<_>>>()?;
    Ok(pieces.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Args, FromArgMatches};

    fn command() -> clap::Command {
        Options::augment_args(clap::Command::new("clean"))
    }

    #[test]
    fn clean_options_parse_verbose_and_help() {
        let matches = command().try_get_matches_from(["clean", "-v"]).unwrap();
        let options = Options::from_arg_matches(&matches).unwrap();
        assert!(options.verbose);
    }

    #[test]
    fn clean_options_reject_unknown_flags() {
        assert!(command()
            .try_get_matches_from(["clean", "file.txt"])
            .is_err());
        assert!(command()
            .try_get_matches_from(["clean", "--verbose"])
            .is_err());
    }
}
