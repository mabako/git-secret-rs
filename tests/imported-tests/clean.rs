use std::fs;
use std::path::Path;
use std::process::{Command, Output};

#[path = "../support/mod.rs"]
mod support;

use support::{
    assert_failure, assert_stdout_contains, assert_success, git_secret, run_success, TempRepo,
};

const FIRST_FILE: &str = "space file";
const SECOND_FILE: &str = "space file two";

#[test]
fn clean_runs_normally() {
    let context = clean_context();

    let output = git_secret_clean(context.repo.path(), &[]);

    assert_success(&output);
    assert_no_secret_files(context.repo.path());
}

#[test]
fn clean_rejects_extra_filename() {
    let context = clean_context();

    let output = git_secret_clean(context.repo.path(), &["extra_filename"]);

    assert_failure(&output);
}

#[test]
fn clean_rejects_bad_argument() {
    let context = clean_context();

    let output = git_secret_clean(context.repo.path(), &["-Z"]);

    assert_failure(&output);
}

#[test]
fn clean_verbose_deletes_secret_files_and_prints_them() {
    let context = clean_context();

    let output = git_secret_clean(context.repo.path(), &["-v"]);

    assert_success(&output);
    assert_no_secret_files(context.repo.path());
    assert_stdout_contains(&output, "deleted");
    assert_stdout_contains(&output, &encrypted_name(FIRST_FILE));
    assert_stdout_contains(&output, &encrypted_name(SECOND_FILE));
}

#[test]
fn clean_secrets_verbose_prints_deleted_files() {
    let context = clean_context();

    let output = git_secret(context.repo.path())
        .arg("clean")
        .env("SECRETS_VERBOSE", "1")
        .output()
        .expect("git-secret clean should run");

    assert_success(&output);
    assert_stdout_contains(&output, "deleted:");
}

#[test]
fn clean_secrets_verbose_zero_stays_quiet() {
    let context = clean_context();

    let output = git_secret(context.repo.path())
        .arg("clean")
        .env("SECRETS_VERBOSE", "0")
        .output()
        .expect("git-secret clean should run");

    assert_success(&output);
    assert_stdout_does_not_contain(&output, "cleaning");
    assert_stdout_does_not_contain(&output, "deleted:");
}

struct CleanContext {
    repo: TempRepo,
}

fn clean_context() -> CleanContext {
    let repo = TempRepo::new();
    run_success(Command::new("git").arg("init").arg(repo.path()));
    run_success(git_secret(repo.path()).arg("init").current_dir(repo.path()));

    write_file(repo.path(), FIRST_FILE, "somecontent");
    write_file(repo.path(), SECOND_FILE, "somecontent2");
    assert_success(&git_secret_add(repo.path(), &[FIRST_FILE, SECOND_FILE]));
    write_encrypted_file(repo.path(), FIRST_FILE);
    write_encrypted_file(repo.path(), SECOND_FILE);

    CleanContext { repo }
}

fn git_secret_add(current_dir: &Path, paths: &[&str]) -> Output {
    git_secret(current_dir)
        .arg("add")
        .args(paths)
        .output()
        .expect("git-secret add should run")
}

fn git_secret_clean(current_dir: &Path, args: &[&str]) -> Output {
    git_secret(current_dir)
        .arg("clean")
        .args(args)
        .output()
        .expect("git-secret clean should run")
}

fn write_file(repo: &Path, path: &str, content: &str) {
    fs::write(repo.join(path), content).expect("secret should be written");
}

fn write_encrypted_file(repo: &Path, path: &str) {
    fs::write(repo.join(encrypted_name(path)), "encrypted")
        .expect("encrypted file should be written");
}

fn encrypted_name(path: &str) -> String {
    format!("{path}.secret")
}

fn assert_no_secret_files(path: &Path) {
    let mut secret_files = Vec::new();
    collect_secret_files(path, &mut secret_files);
    assert!(
        secret_files.is_empty(),
        "secret files should be deleted:\n{}",
        secret_files
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

fn collect_secret_files(path: &Path, secret_files: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(path).expect("directory should be readable") {
        let entry = entry.expect("directory entry should be readable");
        let path = entry.path();
        if path.is_dir() {
            collect_secret_files(&path, secret_files);
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".secret"))
        {
            secret_files.push(path);
        }
    }
}

fn assert_stdout_does_not_contain(output: &Output, unexpected: &str) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(unexpected),
        "stdout should not contain {unexpected:?}:\n{stdout}"
    );
}
