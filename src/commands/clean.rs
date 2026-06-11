use std::fs;
use std::path::{Path, PathBuf};

use crate::git::{ensure_initialized, Repo};
use crate::AppResult;

pub(crate) struct Options {
    verbose: bool,
    help: bool,
}

impl Options {
    pub(crate) fn parse(args: Vec<String>) -> AppResult<Self> {
        let mut verbose = false;
        let mut help = false;

        for arg in args {
            match arg.as_str() {
                "-v" => verbose = true,
                "-h" | "--help" => help = true,
                _ => return Err(format!("unknown clean option '{}'", arg)),
            }
        }

        Ok(Self { verbose, help })
    }
}

pub(crate) fn run(options: Options) -> AppResult<()> {
    if options.help {
        print_help();
        return Ok(());
    }

    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;
    let secret_files = secret_files(repo.root())?;

    for path in secret_files {
        fs::remove_file(&path).map_err(|e| format!("remove {}: {}", path.display(), e))?;
        if options.verbose {
            println!("removed {}", repo_relative_path(repo.root(), &path)?);
        }
    }

    Ok(())
}

fn secret_files(root: &Path) -> AppResult<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_secret_files(root, &mut files)?;
    Ok(files)
}

fn collect_secret_files(path: &Path, files: &mut Vec<PathBuf>) -> AppResult<()> {
    for entry in fs::read_dir(path).map_err(|e| format!("read {}: {}", path.display(), e))? {
        let entry = entry.map_err(|e| format!("read {} entry: {}", path.display(), e))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|e| format!("read {} type: {}", path.display(), e))?;
        if file_type.is_dir() {
            if entry.file_name() != ".git" {
                collect_secret_files(&path, files)?;
            }
        } else if file_type.is_file()
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".secret"))
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

fn print_help() {
    println!(
        "git-secret-clean - deletes all files in the current git-secret repo that end with .secret.\n\
\n\
Usage:\n\
  git secret clean [-v] [-h]\n\
\n\
Options:\n\
  -v  verbose mode, shows which files are deleted\n\
  -h  shows this help"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_options_parse_verbose_and_help() {
        let options = Options::parse(vec!["-v".to_string(), "-h".to_string()]).unwrap();
        assert!(options.verbose);
        assert!(options.help);
    }

    #[test]
    fn clean_options_reject_unknown_flags() {
        assert!(Options::parse(vec!["file.txt".to_string()]).is_err());
        assert!(Options::parse(vec!["--verbose".to_string()]).is_err());
    }
}
