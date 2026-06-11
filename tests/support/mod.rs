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
    path
}

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

pub(crate) fn fixture_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(path)
}
