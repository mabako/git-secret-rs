use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::git::{ensure_initialized, Repo};
use crate::gpg::{gpg_arg_path, user_gpg, UserGpgOptions};
use crate::mapping::Mapping;
use crate::paths::{encrypted_path, selected_paths};
use crate::AppResult;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(clap::Args)]
pub(crate) struct Options {
    #[command(flatten)]
    gpg: UserGpgOptions,
    #[arg(value_name = "filename")]
    paths: Vec<PathBuf>,
}

pub(crate) fn run(options: Options) -> AppResult<()> {
    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;
    let mapping = Mapping::load(&repo)?;
    let paths = selected_paths(&repo, &mapping, options.paths)?;

    for path in paths {
        print_changes(&options.gpg, &repo, &path)?;
    }

    Ok(())
}

fn print_changes(gpg: &UserGpgOptions, repo: &Repo, path: &str) -> AppResult<()> {
    let plaintext = repo.join(path);
    let secret = encrypted_path(repo, path);

    if !plaintext.is_file() {
        return Err(format!("file not found: {}", plaintext.display()));
    }
    if !secret.is_file() {
        return Err(format!("encrypted file not found: {}", secret.display()));
    }

    let decrypted = decrypt_secret(gpg, &secret)?;
    println!("changes in {}:", repo.join(path).display());
    print_diff(&decrypted, &plaintext)?;
    Ok(())
}

fn decrypt_secret(gpg: &UserGpgOptions, secret: &Path) -> AppResult<Vec<u8>> {
    let output = user_gpg(gpg)
        .arg("--batch")
        .arg("--decrypt")
        .arg(gpg_arg_path(secret))
        .output()
        .map_err(|e| format!("decrypt {}: {}", secret.display(), e))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(format!(
            "decrypt {}: gpg exited with {}",
            secret.display(),
            output.status
        ))
    }
}

fn print_diff(decrypted: &[u8], plaintext: &Path) -> AppResult<()> {
    let decrypted_file = TempFile::write(decrypted)?;
    let output = run_diff(decrypted_file.path(), plaintext)?;

    if output.status.success() || output.status.code() == Some(1) {
        io::stdout()
            .write_all(&output.stdout)
            .map_err(|e| format!("write diff output: {}", e))?;
        return Ok(());
    }

    Err(format!(
        "diff {} {}: command exited with {}\n{}",
        decrypted_file.path().display(),
        plaintext.display(),
        output.status,
        String::from_utf8_lossy(&output.stderr)
    ))
}

fn run_diff(left: &Path, right: &Path) -> AppResult<Output> {
    run_diff_command(Path::new("diff"), left, right).or_else(|error| {
        if error.kind() != ErrorKind::NotFound {
            return Err(format!("run diff: {}", error));
        }

        run_git_diff_command(left, right)
    })
}

fn run_diff_command(program: &Path, left: &Path, right: &Path) -> io::Result<Output> {
    Command::new(program)
        .arg("-u")
        .arg(left)
        .arg(right)
        .output()
}

#[cfg(windows)]
fn run_git_diff_command(left: &Path, right: &Path) -> AppResult<Output> {
    let mut last_error = None;
    for program in git_diff_candidates() {
        match run_diff_command(&program, left, right) {
            Ok(output) => return Ok(output),
            Err(error) if error.kind() == ErrorKind::NotFound => last_error = Some(error),
            Err(error) => return Err(format!("run {}: {}", program.display(), error)),
        }
    }

    Err(format!(
        "run diff: diff was not found on PATH and Git for Windows diff.exe was not found{}",
        last_error
            .map(|error| format!(": {}", error))
            .unwrap_or_default()
    ))
}

#[cfg(not(windows))]
fn run_git_diff_command(_left: &Path, _right: &Path) -> AppResult<Output> {
    Err("run diff: diff was not found on PATH".to_string())
}

#[cfg(windows)]
fn git_diff_candidates() -> Vec<PathBuf> {
    ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"]
        .into_iter()
        .filter_map(env::var_os)
        .map(|root| {
            PathBuf::from(root)
                .join("Git")
                .join("usr")
                .join("bin")
                .join("diff.exe")
        })
        .collect()
}

struct TempFile {
    path: PathBuf,
}

impl TempFile {
    fn write(contents: &[u8]) -> AppResult<Self> {
        for _ in 0..100 {
            let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = env::temp_dir().join(format!(
                "git-secret-changes-{}-{:x}",
                std::process::id(),
                counter
            ));

            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut file) => {
                    file.write_all(contents)
                        .map_err(|e| format!("write {}: {}", path.display(), e))?;
                    return Ok(Self { path });
                }
                Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(format!("create {}: {}", path.display(), error)),
            }
        }

        Err("create temporary decrypted file: no unique path available".to_string())
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Args, FromArgMatches};
    use std::path::PathBuf;

    fn command() -> clap::Command {
        Options::augment_args(clap::Command::new("changes"))
    }

    #[test]
    fn changes_options_parse_homedir_password_and_paths() {
        let matches = command()
            .try_get_matches_from(["changes", "-d", "keys", "-p", "secret", "file.txt"])
            .unwrap();
        let options = Options::from_arg_matches(&matches).unwrap();

        assert_eq!(options.gpg.homedir, Some(PathBuf::from("keys")));
        assert_eq!(options.gpg.passphrase, Some("secret".to_string()));
        assert_eq!(options.paths, vec![PathBuf::from("file.txt")]);
    }

    #[test]
    fn changes_options_require_values() {
        assert!(command().try_get_matches_from(["changes", "-d"]).is_err());
        assert!(command().try_get_matches_from(["changes", "-p"]).is_err());
    }
}
