use std::fs;
use std::path::Path;
use std::process::{Command, Output};

#[path = "../support/mod.rs"]
mod support;

use support::{
    assert_failure, assert_stdout_contains, assert_success, git_secret, run_success, TempRepo,
};

const FILE_TO_HIDE: &str = "file_to_hide";
const SECOND_FILE: &str = "second_file.txt";
const FILE_CONTENTS: &str = "hidden content юникод";

#[test]
fn list_runs_normally() {
    let context = list_context();

    let output = git_secret_list(context.repo.path(), &[]);

    assert_success(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), FILE_TO_HIDE);
}

#[test]
fn list_rejects_extra_filename() {
    let context = list_context();

    let output = git_secret_list(context.repo.path(), &["extra_filename"]);

    assert_failure(&output);
}

#[test]
fn list_rejects_bad_argument() {
    let context = list_context();

    let output = git_secret_list(context.repo.path(), &["-Z"]);

    assert_failure(&output);
}

#[test]
fn list_prints_multiple_files() {
    let context = list_context();
    write_file(context.repo.path(), SECOND_FILE, FILE_CONTENTS);
    assert_success(&git_secret_add(context.repo.path(), &[SECOND_FILE]));

    let output = git_secret_list(context.repo.path(), &[]);

    assert_success(&output);
    assert_stdout_contains(&output, FILE_TO_HIDE);
    assert_stdout_contains(&output, SECOND_FILE);
}

#[test]
fn list_fails_on_empty_repo() {
    let context = list_context();
    assert_success(
        &git_secret(context.repo.path())
            .arg("remove")
            .arg(FILE_TO_HIDE)
            .output()
            .expect("git-secret remove should run"),
    );

    let output = git_secret_list(context.repo.path(), &[]);

    assert_failure(&output);
}

struct ListContext {
    repo: TempRepo,
}

fn list_context() -> ListContext {
    let repo = TempRepo::new();
    run_success(Command::new("git").arg("init").arg(repo.path()));
    run_success(git_secret(repo.path()).arg("init").current_dir(repo.path()));

    write_file(repo.path(), FILE_TO_HIDE, FILE_CONTENTS);
    assert_success(&git_secret_add(repo.path(), &[FILE_TO_HIDE]));

    ListContext { repo }
}

fn git_secret_add(current_dir: &Path, paths: &[&str]) -> Output {
    git_secret(current_dir)
        .arg("add")
        .args(paths)
        .output()
        .expect("git-secret add should run")
}

fn git_secret_list(current_dir: &Path, args: &[&str]) -> Output {
    git_secret(current_dir)
        .arg("list")
        .args(args)
        .output()
        .expect("git-secret list should run")
}

fn write_file(repo: &Path, path: &str, content: &str) {
    fs::write(repo.join(path), content).expect("secret should be written");
}
