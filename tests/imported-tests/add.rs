use std::fs;
use std::path::Path;
use std::process::{Command, Output};

#[path = "../support/mod.rs"]
mod support;

use support::{assert_failure, assert_success, git_secret, run_success, TempRepo};

const DEFAULT_FILENAME: &str = "space file";
const SECOND_FILENAME: &str = "space file two";

#[test]
fn add_runs_normally() {
    let context = add_context();
    write_file(context.repo.path(), DEFAULT_FILENAME, "content");

    let output = git_secret_add(context.repo.path(), &[DEFAULT_FILENAME]);

    assert_success(&output);
    assert_mapping_paths(context.repo.path(), &[DEFAULT_FILENAME]);
}

#[test]
fn add_rejects_bad_argument() {
    let context = add_context();
    write_file(context.repo.path(), DEFAULT_FILENAME, "content");

    let output = git_secret(context.repo.path())
        .arg("add")
        .arg("-Z")
        .arg(DEFAULT_FILENAME)
        .output()
        .expect("git-secret add should run");

    assert_failure(&output);
}

#[test]
fn add_ignores_file_by_default() {
    let context = add_context();
    write_file(context.repo.path(), DEFAULT_FILENAME, "content");

    let output = git_secret_add(context.repo.path(), &[DEFAULT_FILENAME]);

    assert_success(&output);
    assert_git_check_ignore(context.repo.path(), DEFAULT_FILENAME);
}

#[test]
fn add_i_is_noop_and_updates_root_gitignore() {
    let context = add_context();
    write_file(context.repo.path(), DEFAULT_FILENAME, "content");

    let output = git_secret(context.repo.path())
        .arg("add")
        .arg("-i")
        .arg(DEFAULT_FILENAME)
        .output()
        .expect("git-secret add should run");

    assert_success(&output);
    assert_gitignore_lines(
        context.repo.path(),
        &[
            ".gitsecret/keys/random_seed",
            ".gitsecret/keys/*.lock",
            "!*.secret",
            DEFAULT_FILENAME,
        ],
    );
}

#[test]
fn add_i_from_subfolder_updates_root_gitignore() {
    let context = add_context();
    let nested_dir = context.repo.path().join("test_dir").join("adding");
    fs::create_dir_all(&nested_dir).expect("nested add directory should be created");
    fs::write(nested_dir.join("test_file.auto_ignore"), "content")
        .expect("nested secret should be written");

    let output = git_secret(&nested_dir)
        .arg("add")
        .arg("-i")
        .arg("test_file.auto_ignore")
        .output()
        .expect("git-secret add should run");

    assert_success(&output);
    assert_gitignore_contains(context.repo.path(), "test_dir/adding/test_file.auto_ignore");
    assert!(
        !nested_dir.join(".gitignore").exists(),
        "nested .gitignore should not be created"
    );
}

#[test]
fn add_accepts_relative_path_from_sibling_directory() {
    let context = add_context();
    let root = context.repo.path().join("test_dir");
    let node = root.join("node");
    let sibling = root.join("sibling");
    fs::create_dir_all(&node).expect("node directory should be created");
    fs::create_dir_all(&sibling).expect("sibling directory should be created");
    fs::write(node.join(DEFAULT_FILENAME), "content").expect("secret should be written");

    let output = git_secret(&sibling)
        .arg("add")
        .arg(format!("../node/{DEFAULT_FILENAME}"))
        .output()
        .expect("git-secret add should run");

    assert_success(&output);
    assert_stdout_contains(&output, "git-secret: 1 item(s) added.");
    assert_mapping_paths(
        context.repo.path(),
        &[&format!("test_dir/node/{DEFAULT_FILENAME}")],
    );
}

#[test]
fn add_accepts_file_in_subfolder() {
    let context = add_context();
    let test_dir = context.repo.path().join("test_dir");
    fs::create_dir_all(&test_dir).expect("test directory should be created");
    fs::write(test_dir.join(DEFAULT_FILENAME), "content").expect("secret should be written");

    let output = git_secret_add(
        context.repo.path(),
        &[&format!("test_dir/{DEFAULT_FILENAME}")],
    );

    assert_success(&output);
    assert_stdout_contains(&output, "git-secret: 1 item(s) added.");
}

#[test]
fn add_twice_reports_zero_new_items() {
    let context = add_context();
    write_file(context.repo.path(), DEFAULT_FILENAME, "content");

    assert_success(&git_secret_add(context.repo.path(), &[DEFAULT_FILENAME]));
    let output = git_secret_add(context.repo.path(), &[DEFAULT_FILENAME]);

    assert_success(&output);
    assert_stdout_contains(&output, "git-secret: 0 item(s) added.");
    assert_mapping_paths(context.repo.path(), &[DEFAULT_FILENAME]);
}

#[test]
fn add_multiple_files_updates_gitignore() {
    let context = add_context();
    write_file(context.repo.path(), DEFAULT_FILENAME, "content1");
    write_file(context.repo.path(), SECOND_FILENAME, "content2");

    let output = git_secret_add(context.repo.path(), &[DEFAULT_FILENAME, SECOND_FILENAME]);

    assert_success(&output);
    assert_stdout_contains(&output, "git-secret: 2 item(s) added.");
    assert_gitignore_contains(context.repo.path(), DEFAULT_FILENAME);
    assert_gitignore_contains(context.repo.path(), SECOND_FILENAME);
    assert_eq!(gitignore_lines(context.repo.path()).len(), 5);
}

#[test]
fn add_verbose_prints_each_added_file() {
    let context = add_context();
    write_file(context.repo.path(), DEFAULT_FILENAME, "content1");
    write_file(context.repo.path(), SECOND_FILENAME, "content2");

    let output = git_secret(context.repo.path())
        .arg("add")
        .arg("-v")
        .arg(DEFAULT_FILENAME)
        .arg(SECOND_FILENAME)
        .output()
        .expect("git-secret add should run");

    assert_success(&output);
    assert_stdout_contains(
        &output,
        &format!("git-secret: adding file: {DEFAULT_FILENAME}"),
    );
    assert_stdout_contains(
        &output,
        &format!("git-secret: adding file: {SECOND_FILENAME}"),
    );
    assert_stdout_contains(&output, "git-secret: 2 item(s) added.");
}

#[cfg(not(windows))]
#[test]
fn add_accepts_file_with_special_chars() {
    const SPECIAL_FILENAME: &str = "space file three [] * $";

    let context = add_context();
    write_file(context.repo.path(), SPECIAL_FILENAME, "content");

    let output = git_secret_add(context.repo.path(), &[SPECIAL_FILENAME]);

    assert_success(&output);
    assert_mapping_paths(context.repo.path(), &[SPECIAL_FILENAME]);
    assert_git_check_ignore(context.repo.path(), SPECIAL_FILENAME);
}

struct AddContext {
    repo: TempRepo,
}

fn add_context() -> AddContext {
    let repo = TempRepo::new();
    run_success(Command::new("git").arg("init").arg(repo.path()));
    run_success(git_secret(repo.path()).arg("init").current_dir(repo.path()));

    AddContext { repo }
}

fn git_secret_add(current_dir: &Path, paths: &[&str]) -> Output {
    git_secret(current_dir)
        .arg("add")
        .args(paths)
        .output()
        .expect("git-secret add should run")
}

fn write_file(repo: &Path, path: &str, content: &str) {
    fs::write(repo.join(path), content).expect("secret should be written");
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

fn assert_mapping_paths(repo: &Path, expected: &[&str]) {
    assert_eq!(
        mapping_paths(repo),
        expected
            .iter()
            .map(|path| path.to_string())
            .collect::<Vec<_>>()
    );
}

fn gitignore_lines(repo: &Path) -> Vec<String> {
    fs::read_to_string(repo.join(".gitignore"))
        .expect(".gitignore should be readable")
        .lines()
        .map(str::to_string)
        .collect()
}

fn assert_gitignore_lines(repo: &Path, expected: &[&str]) {
    assert_eq!(
        gitignore_lines(repo),
        expected
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
    );
}

fn assert_gitignore_contains(repo: &Path, expected: &str) {
    let lines = gitignore_lines(repo);
    assert!(
        lines.iter().any(|line| line == expected),
        ".gitignore should contain {expected:?}:\n{}",
        lines.join("\n")
    );
}

fn assert_git_check_ignore(repo: &Path, path: &str) {
    let output = Command::new("git")
        .arg("check-ignore")
        .arg(path)
        .current_dir(repo)
        .output()
        .expect("git check-ignore should run");
    assert_success(&output);
}

fn assert_stdout_contains(output: &Output, expected: &str) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(expected),
        "stdout should contain {expected:?}:\n{stdout}"
    );
}
