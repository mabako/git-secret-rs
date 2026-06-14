use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output};

#[path = "../support/mod.rs"]
mod support;

use support::{
    assert_failure, assert_stdout_contains, assert_success, fixture_key_passphrase, git_secret,
    import_private_fixture_key, import_public_fixture_key, run_success, TempDir, TempRepo,
};

const USER1_EMAIL: &str = "user1@gitsecret.io";
const FILE_TO_HIDE: &str = "space file";
const SECOND_FILE_TO_HIDE: &str = "space file two";
const THIRD_FILE_TO_HIDE: &str = "space file three";
const FILE_NON_EXISTENT: &str = "NO-SUCH-FILE";
const FILE_CONTENTS: &str = "hidden content юникод";

struct ChangesContext {
    repo: TempRepo,
    gpg_home: TempDir,
}

#[test]
fn changes_one_file_with_no_file_changed() {
    let context = changes_context();

    let output = git_secret_changes(
        context.repo.path(),
        context.gpg_home.path(),
        &[FILE_TO_HIDE],
    );

    assert_success(&output);
    assert_eq!(stdout_line_count(&output), 1);
}

#[test]
fn changes_one_file_changed() {
    let context = changes_context();
    append_file(context.repo.path(), FILE_TO_HIDE, "new content\n");

    let output = git_secret_changes(
        context.repo.path(),
        context.gpg_home.path(),
        &[FILE_TO_HIDE],
    );

    assert_success(&output);
    assert_stdout_contains_path(
        &output,
        &format!("changes in {}", full_path(&context, FILE_TO_HIDE)),
    );
    assert_stdout_contains(&output, FILE_CONTENTS);
    assert_stdout_contains(&output, "+new content");
    assert_eq!(stdout_line_count(&output), 6);
}

#[test]
fn changes_fails_with_source_file_missing() {
    let context = changes_context();
    fs::remove_file(context.repo.path().join(FILE_TO_HIDE)).expect("plaintext should be removed");

    let output = git_secret_changes(
        context.repo.path(),
        context.gpg_home.path(),
        &[FILE_TO_HIDE],
    );

    assert_failure(&output);
}

#[test]
fn changes_fails_with_hidden_file_missing() {
    let context = changes_context();
    fs::remove_file(encrypted_path(context.repo.path(), FILE_TO_HIDE))
        .expect("encrypted file should be removed");

    let output = git_secret_changes(
        context.repo.path(),
        context.gpg_home.path(),
        &[FILE_TO_HIDE],
    );

    assert_failure(&output);
}

#[test]
fn changes_one_file_changed_with_deletions() {
    let context = changes_context();
    write_file(context.repo.path(), FILE_TO_HIDE, "replace\n");

    let output = git_secret_changes(
        context.repo.path(),
        context.gpg_home.path(),
        &[FILE_TO_HIDE],
    );

    assert_success(&output);
    assert_stdout_contains_path(
        &output,
        &format!("changes in {}", full_path(&context, FILE_TO_HIDE)),
    );
    assert_stdout_contains(&output, &format!("-{FILE_CONTENTS}"));
    assert_stdout_contains(&output, "+replace");
}

#[test]
fn changes_two_files_with_no_file_changed() {
    let context = changes_context();

    let output = git_secret_changes(context.repo.path(), context.gpg_home.path(), &[]);

    assert_success(&output);
    assert_eq!(stdout_line_count(&output), 2);
}

#[test]
fn changes_multiple_files_changed() {
    let context = changes_context();
    append_file(context.repo.path(), FILE_TO_HIDE, "new content\n");
    append_file(
        context.repo.path(),
        SECOND_FILE_TO_HIDE,
        "something different\n",
    );

    let output = git_secret_changes(context.repo.path(), context.gpg_home.path(), &[]);

    assert_success(&output);
    assert_stdout_contains_path(
        &output,
        &format!("changes in {}", full_path(&context, FILE_TO_HIDE)),
    );
    assert_stdout_contains(&output, "+new content");
    assert_stdout_contains_path(
        &output,
        &format!("changes in {}", full_path(&context, SECOND_FILE_TO_HIDE)),
    );
    assert_stdout_contains(&output, "+something different");
}

#[test]
fn changes_multiple_selected_files_changed() {
    let context = changes_context();
    append_file(context.repo.path(), FILE_TO_HIDE, "new content\n");
    append_file(
        context.repo.path(),
        SECOND_FILE_TO_HIDE,
        "something different\n",
    );

    let output = git_secret_changes(
        context.repo.path(),
        context.gpg_home.path(),
        &[FILE_TO_HIDE, SECOND_FILE_TO_HIDE],
    );

    assert_success(&output);
    assert_stdout_contains_path(
        &output,
        &format!("changes in {}", full_path(&context, FILE_TO_HIDE)),
    );
    assert_stdout_contains(&output, "+new content");
    assert_stdout_contains_path(
        &output,
        &format!("changes in {}", full_path(&context, SECOND_FILE_TO_HIDE)),
    );
    assert_stdout_contains(&output, "+something different");
}

#[test]
fn changes_fails_on_file_that_does_not_exist() {
    let context = changes_context();

    let output = git_secret_changes(
        context.repo.path(),
        context.gpg_home.path(),
        &[FILE_NON_EXISTENT],
    );

    assert_failure(&output);
}

#[test]
fn changes_one_file_without_newlines() {
    let context = changes_context();
    write_file(context.repo.path(), THIRD_FILE_TO_HIDE, FILE_CONTENTS);
    assert_success(&git_secret_add(context.repo.path(), &[THIRD_FILE_TO_HIDE]));
    assert_success(&git_secret_hide(context.repo.path()));

    let output = git_secret_changes(
        context.repo.path(),
        context.gpg_home.path(),
        &[THIRD_FILE_TO_HIDE],
    );

    assert_success(&output);
    assert_eq!(stdout_line_count(&output), 1);
}

#[test]
fn changes_rejects_bad_argument() {
    let context = changes_context();

    let output = git_secret(context.repo.path())
        .arg("changes")
        .arg("-Z")
        .output()
        .expect("git-secret changes should run");

    assert_failure(&output);
}

fn changes_context() -> ChangesContext {
    let repo = TempRepo::new();
    run_success(Command::new("git").arg("init").arg(repo.path()));
    run_success(git_secret(repo.path()).arg("init").current_dir(repo.path()));

    import_public_fixture_key(&repo.path().join(".gitsecret").join("keys"), USER1_EMAIL);

    let gpg_home = TempDir::new();
    import_private_fixture_key(gpg_home.path(), USER1_EMAIL);

    write_file(repo.path(), FILE_TO_HIDE, &format!("{FILE_CONTENTS}\n"));
    write_file(
        repo.path(),
        SECOND_FILE_TO_HIDE,
        &format!("{FILE_CONTENTS}\n"),
    );
    assert_success(&git_secret_add(
        repo.path(),
        &[FILE_TO_HIDE, SECOND_FILE_TO_HIDE],
    ));
    assert_success(&git_secret_hide(repo.path()));

    ChangesContext { repo, gpg_home }
}

fn git_secret_add(current_dir: &Path, paths: &[&str]) -> Output {
    git_secret(current_dir)
        .arg("add")
        .args(paths)
        .output()
        .expect("git-secret add should run")
}

fn git_secret_hide(current_dir: &Path) -> Output {
    git_secret(current_dir)
        .arg("hide")
        .output()
        .expect("git-secret hide should run")
}

fn git_secret_changes(current_dir: &Path, gpg_home: &Path, paths: &[&str]) -> Output {
    git_secret(current_dir)
        .arg("changes")
        .arg("-d")
        .arg(gpg_home)
        .arg("-p")
        .arg(fixture_key_passphrase(USER1_EMAIL))
        .args(paths)
        .output()
        .expect("git-secret changes should run")
}

fn write_file(repo: &Path, path: &str, content: &str) {
    fs::write(repo.join(path), content).expect("secret should be written");
}

fn append_file(repo: &Path, path: &str, content: &str) {
    OpenOptions::new()
        .append(true)
        .open(repo.join(path))
        .expect("secret should open for append")
        .write_all(content.as_bytes())
        .expect("secret append should be written");
}

fn encrypted_path(repo: &Path, path: &str) -> std::path::PathBuf {
    repo.join(format!("{path}.secret"))
}

fn full_path(context: &ChangesContext, path: &str) -> String {
    context.repo.path().join(path).display().to_string()
}

fn stdout_line_count(output: &Output) -> usize {
    String::from_utf8_lossy(&output.stdout).lines().count()
}

fn assert_stdout_contains_path(output: &Output, expected: &str) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stdout = normalize_slashes(&stdout);
    let expected = normalize_slashes(expected);
    assert!(
        stdout.contains(&expected),
        "stdout should contain {expected:?}:\n{stdout}"
    );
}

fn normalize_slashes(value: &str) -> String {
    value.replace('\\', "/").replace("/private/var/", "/var/")
}
