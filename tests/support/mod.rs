#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) struct TempRepo {
    path: PathBuf,
}

pub(crate) struct TempDir {
    path: PathBuf,
}

impl TempRepo {
    pub(crate) fn new(prefix: &str) -> Self {
        Self {
            path: create_temp_path(prefix),
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

impl TempDir {
    pub(crate) fn new(prefix: &str) -> Self {
        Self {
            path: create_temp_path(prefix),
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn create_temp_path(prefix: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos() as u64;
    let path =
        std::env::temp_dir().join(format!("{}-{:x}-{:x}", prefix, std::process::id(), unique));
    fs::create_dir_all(&path).expect("temp directory should be created");
    set_private_directory_permissions(&path);
    path
}

#[cfg(unix)]
fn set_private_directory_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .expect("temp directory permissions should be restricted");
}

#[cfg(not(unix))]
fn set_private_directory_permissions(_path: &Path) {}

pub(crate) fn run_success(command: &mut Command) -> Output {
    let output = command.output().expect("command should run");
    assert!(
        output.status.success(),
        "command failed with {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

pub(crate) fn gpg_command() -> Command {
    if let Some(secrets_gpg_command) = std::env::var_os("SECRETS_GPG_COMMAND") {
        return Command::new(PathBuf::from(secrets_gpg_command));
    }

    if std::env::var("MSYSTEM").ok().as_deref() == Some("MINGW64") {
        if let Some(program_files_x86) = std::env::var_os("ProgramFiles(x86)") {
            return Command::new(
                PathBuf::from(program_files_x86)
                    .join("GnuPG")
                    .join("bin")
                    .join("gpg.exe"),
            );
        }
    }

    Command::new("gpg")
}

pub(crate) fn fixture_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(path)
}
