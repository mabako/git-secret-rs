use std::fs;
use std::path::Path;
use std::process::{Command, Output};

#[path = "../support/mod.rs"]
mod support;

use support::{assert_failure, assert_success, git_secret, run_success, TempDir, TempRepo};

#[test]
fn secrets_dir_env_var_defaults_to_gitsecret() {
    let repo = TempRepo::new();
    run_success(Command::new("git").arg("init").arg(repo.path()));

    let output = git_secret_init(repo.path());

    assert_success(&output);
    assert!(repo.path().join(".gitsecret").is_dir());
}

#[test]
fn init_fails_without_git_repository() {
    let dir = TempDir::new();

    let output = git_secret_init(dir.path());

    assert_failure(&output);
}

#[test]
fn init_runs_normally() {
    let repo = TempRepo::new();
    run_success(Command::new("git").arg("init").arg(repo.path()));

    let output = git_secret_init(repo.path());

    assert_success(&output);
    assert!(repo.path().join(".gitsecret").is_dir());
}

#[test]
fn init_rejects_extra_filename() {
    let repo = TempRepo::new();
    run_success(Command::new("git").arg("init").arg(repo.path()));

    let output = git_secret(repo.path())
        .arg("init")
        .arg("extra_filename")
        .output();
    let output = output.expect("git-secret init should run");

    assert_failure(&output);
}

#[test]
fn init_rejects_bad_argument() {
    let repo = TempRepo::new();
    run_success(Command::new("git").arg("init").arg(repo.path()));

    let output = git_secret(repo.path()).arg("init").arg("-Z").output();
    let output = output.expect("git-secret init should run");

    assert_failure(&output);
}

#[test]
fn init_from_subdirectory_creates_secret_dir_at_repo_root() {
    let repo = TempRepo::new();
    run_success(Command::new("git").arg("init").arg(repo.path()));
    let nested_dir = repo.path().join("test_dir").join("nested").join("dirs");
    fs::create_dir_all(&nested_dir).expect("nested test directory should be created");

    let output = git_secret_init(&nested_dir);

    assert_success(&output);
    assert!(!nested_dir.join(".gitsecret").exists());
    assert!(repo.path().join(".gitsecret").is_dir());
}

#[test]
fn init_works_when_parent_path_contains_spaces() {
    let temp = TempDir::new();
    let repo = temp.path().join("path with spaces");
    fs::create_dir_all(&repo).expect("spaced repository directory should be created");
    run_success(Command::new("git").arg("init").arg(&repo));

    let output = git_secret_init(&repo);

    assert_success(&output);
    assert!(repo.join(".gitsecret").is_dir());
}

#[test]
fn init_fails_when_gitsecret_directory_already_exists_without_metadata() {
    let repo = TempRepo::new();
    run_success(Command::new("git").arg("init").arg(repo.path()));
    fs::create_dir(repo.path().join(".gitsecret")).expect(".gitsecret should be created");

    let output = git_secret_init(repo.path());

    assert_failure(&output);
    assert_eq!(
        String::from_utf8_lossy(&output.stderr).trim(),
        "git-secret: abort: already initialized."
    );
}

fn git_secret_init(current_dir: &Path) -> Output {
    git_secret(current_dir)
        .arg("init")
        .output()
        .expect("git-secret init should run")
}
