use std::fs;
use std::path::Path;
use std::process::{Command, Output};

#[path = "../support/mod.rs"]
mod support;

use support::{assert_failure, assert_success, git_secret, run_success, TempRepo};

const FIRST_FILE: &str = "space file";
const SECOND_FILE: &str = "space file two";
const THIRD_FILE: &str = "space file three";

#[test]
fn remove_runs_normally() {
    let context = remove_context();

    let output = git_secret_remove(context.repo.path(), &[SECOND_FILE]);

    assert_success(&output);
    assert_stdout_contains(&output, "removed from index.");
    assert_stdout_contains(
        &output,
        &format!("ensure that files: [{SECOND_FILE}] are now not ignored."),
    );
    assert_mapping_does_not_contain(context.repo.path(), SECOND_FILE);
    assert_mapping_contains(context.repo.path(), FIRST_FILE);
    assert_encrypted_file_exists(context.repo.path(), FIRST_FILE);
    assert_encrypted_file_exists(context.repo.path(), SECOND_FILE);
}

#[test]
fn remove_accepts_multiple_arguments() {
    let context = remove_context();

    let output = git_secret_remove(context.repo.path(), &[FIRST_FILE, SECOND_FILE]);

    assert_success(&output);
    assert_mapping_does_not_contain(context.repo.path(), FIRST_FILE);
    assert_mapping_does_not_contain(context.repo.path(), SECOND_FILE);
    assert_encrypted_file_exists(context.repo.path(), FIRST_FILE);
    assert_encrypted_file_exists(context.repo.path(), SECOND_FILE);
}

#[test]
fn remove_accepts_path_with_slashes() {
    let context = remove_context();
    let folder = context.repo.path().join("somedir");
    fs::create_dir_all(&folder).expect("folder should be created");
    let file_in_folder = format!("somedir/{THIRD_FILE}");
    write_file(context.repo.path(), &file_in_folder, "somecontent3");
    assert_success(&git_secret_add(context.repo.path(), &[&file_in_folder]));
    write_encrypted_file(context.repo.path(), &file_in_folder);

    let output = git_secret_remove(context.repo.path(), &[&file_in_folder]);

    assert_success(&output);
    assert_mapping_does_not_contain(context.repo.path(), &file_in_folder);
    assert_encrypted_file_exists(context.repo.path(), &file_in_folder);
}

#[test]
fn remove_c_deletes_encrypted_file() {
    let context = remove_context();

    let output = git_secret(context.repo.path())
        .arg("remove")
        .arg("-c")
        .arg(SECOND_FILE)
        .output()
        .expect("git-secret remove should run");

    assert_success(&output);
    assert_mapping_does_not_contain(context.repo.path(), SECOND_FILE);
    assert_encrypted_file_exists(context.repo.path(), FIRST_FILE);
    assert_encrypted_file_does_not_exist(context.repo.path(), SECOND_FILE);
}

#[test]
fn remove_rejects_bad_argument() {
    let context = remove_context();

    let output = git_secret(context.repo.path())
        .arg("remove")
        .arg("-Z")
        .arg(SECOND_FILE)
        .output()
        .expect("git-secret remove should run");

    assert_failure(&output);
}

struct RemoveContext {
    repo: TempRepo,
}

fn remove_context() -> RemoveContext {
    let repo = TempRepo::new();
    run_success(Command::new("git").arg("init").arg(repo.path()));
    run_success(git_secret(repo.path()).arg("init").current_dir(repo.path()));

    write_file(repo.path(), FIRST_FILE, "somecontent");
    write_file(repo.path(), SECOND_FILE, "somecontent2");
    assert_success(&git_secret_add(repo.path(), &[FIRST_FILE, SECOND_FILE]));
    write_encrypted_file(repo.path(), FIRST_FILE);
    write_encrypted_file(repo.path(), SECOND_FILE);

    RemoveContext { repo }
}

fn git_secret_add(current_dir: &Path, paths: &[&str]) -> Output {
    git_secret(current_dir)
        .arg("add")
        .args(paths)
        .output()
        .expect("git-secret add should run")
}

fn git_secret_remove(current_dir: &Path, paths: &[&str]) -> Output {
    git_secret(current_dir)
        .arg("remove")
        .args(paths)
        .output()
        .expect("git-secret remove should run")
}

fn write_file(repo: &Path, path: &str, content: &str) {
    fs::write(repo.join(path), content).expect("secret should be written");
}

fn write_encrypted_file(repo: &Path, path: &str) {
    let encrypted = encrypted_path(repo, path);
    if let Some(parent) = encrypted.parent() {
        fs::create_dir_all(parent).expect("encrypted file parent should be created");
    }
    fs::write(encrypted, "encrypted").expect("encrypted file should be written");
}

fn encrypted_path(repo: &Path, path: &str) -> std::path::PathBuf {
    repo.join(format!("{path}.secret"))
}

fn mapping_paths(repo: &Path) -> Vec<String> {
    let mapping = fs::read_to_string(repo.join(".gitsecret/paths/mapping.cfg"))
        .expect("mapping should be readable");
    mapping
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            line.rsplit_once(':')
                .map(|(path, _)| path)
                .unwrap_or(line)
                .to_string()
        })
        .collect()
}

fn assert_mapping_contains(repo: &Path, expected: &str) {
    let paths = mapping_paths(repo);
    assert!(
        paths.iter().any(|path| path == expected),
        "mapping should contain {expected:?}:\n{}",
        paths.join("\n")
    );
}

fn assert_mapping_does_not_contain(repo: &Path, unexpected: &str) {
    let paths = mapping_paths(repo);
    assert!(
        !paths.iter().any(|path| path == unexpected),
        "mapping should not contain {unexpected:?}:\n{}",
        paths.join("\n")
    );
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

fn assert_stdout_contains(output: &Output, expected: &str) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(expected),
        "stdout should contain {expected:?}:\n{stdout}"
    );
}
