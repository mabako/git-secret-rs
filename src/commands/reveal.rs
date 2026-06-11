use std::fs;
use std::path::PathBuf;

use crate::git::{ensure_initialized, gpg, Repo};
use crate::mapping::Mapping;
use crate::paths::{encrypted_path, selected_paths};
use crate::process::CommandExt;
use crate::AppResult;

pub(crate) struct Options {
    force: bool,
    paths: Vec<PathBuf>,
}

impl Options {
    pub(crate) fn parse(args: Vec<String>) -> AppResult<Self> {
        let mut force = false;
        let mut paths = Vec::new();

        for arg in args {
            match arg.as_str() {
                "-f" | "--force" => force = true,
                _ if arg.starts_with('-') => {
                    return Err(format!("unknown reveal option '{}'", arg))
                }
                _ => paths.push(PathBuf::from(arg)),
            }
        }

        Ok(Self { force, paths })
    }
}

pub(crate) fn run(options: Options) -> AppResult<()> {
    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;
    let mapping = Mapping::load(&repo)?;
    let paths = selected_paths(&mapping, options.paths)?;

    for path in paths {
        let input = encrypted_path(&repo, &path);
        let output = repo.join(&path);

        if !input.is_file() {
            return Err(format!("{} does not exist", input.display()));
        }
        if output.exists() && !options.force {
            return Err(format!(
                "{} already exists; pass --force to overwrite",
                output.display()
            ));
        }

        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("create {}: {}", parent.display(), e))?;
        }

        gpg(&repo)
            .arg("--batch")
            .arg("--yes")
            .arg("--decrypt")
            .arg("--output")
            .arg(&output)
            .arg(&input)
            .status_ok(&format!("decrypt {}", path))?;
        println!("decrypted {}", path);
    }

    Ok(())
}
