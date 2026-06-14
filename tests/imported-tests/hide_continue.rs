use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[path = "../support/mod.rs"]
mod support;

use support::{assert_success, git_secret, import_public_fixture_key, run_success, TempRepo};

const USER1_EMAIL: &str = "user1@gitsecret.io";
const FILE_TO_HIDE: &str = "space file";
const SECOND_FILE_TO_HIDE: &str = "space file two";
const FILE_CONTENTS: &str = "hidden content юникод";

#[test]
fn hide_continue_skips_missing_input_file() {
    let context = hide_continue_context();
    fs::rename(
        context.repo.path().join(FILE_TO_HIDE),
        context.repo.path().join(format!("{FILE_TO_HIDE}.was")),
    )
    .expect("first file should be moved out of the way");

    let output = git_secret_hide(context.repo.path(), &["-F"]);

    assert_success(&output);
    assert_done(&output, 1, 2);
    assert_encrypted_file_does_not_exist(context.repo.path(), FILE_TO_HIDE);
    assert_encrypted_file_exists(context.repo.path(), SECOND_FILE_TO_HIDE);
}

struct HideContinueContext {
    repo: TempRepo,
}

fn hide_continue_context() -> HideContinueContext {
    let repo = TempRepo::new("imported-hide-continue");
    run_success(Command::new("git").arg("init").arg(repo.path()));
    run_success(git_secret(repo.path()).arg("init").current_dir(repo.path()));

    import_public_fixture_key(&repo.path().join(".gitsecret").join("keys"), USER1_EMAIL);

    write_file(repo.path(), FILE_TO_HIDE, FILE_CONTENTS);
    write_file(repo.path(), SECOND_FILE_TO_HIDE, FILE_CONTENTS);
    assert_success(&git_secret_add(
        repo.path(),
        &[FILE_TO_HIDE, SECOND_FILE_TO_HIDE],
    ));

    HideContinueContext { repo }
}

fn git_secret_add(current_dir: &Path, paths: &[&str]) -> Output {
    git_secret(current_dir)
        .arg("add")
        .args(paths)
        .output()
        .expect("git-secret add should run")
}

fn git_secret_hide(current_dir: &Path, args: &[&str]) -> Output {
    git_secret(current_dir)
        .arg("hide")
        .args(args)
        .output()
        .expect("git-secret hide should run")
}

fn write_file(repo: &Path, path: &str, content: &str) {
    fs::write(repo.join(path), content).expect("secret should be written");
}

fn encrypted_path(repo: &Path, path: &str) -> PathBuf {
    repo.join(format!("{path}.secret"))
}

fn assert_encrypted_file_exists(repo: &Path, path: &str) {
    let encrypted = encrypted_path(repo, path);
    assert!(encrypted.is_file(), "{} should exist", encrypted.display());
}

fn assert_encrypted_file_does_not_exist(repo: &Path, path: &str) {
    let encrypted = encrypted_path(repo, path);
    assert!(
        !encrypted.exists(),
        "{} should not exist",
        encrypted.display()
    );
}

fn assert_done(output: &Output, hidden: usize, total: usize) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected = format!("git-secret: done. {hidden} of {total} files are hidden.");
    assert!(
        stdout.contains(&expected),
        "stdout should contain {expected:?}:\n{stdout}"
    );
}
