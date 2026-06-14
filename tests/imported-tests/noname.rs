use std::fs;
use std::path::Path;
use std::process::{Command, Output};

#[path = "../support/mod.rs"]
mod support;

use support::{
    assert_stdout_contains, assert_success, git_secret, import_public_fixture_key, run_success,
    TempDir, TempRepo,
};

const NONAME_USER_EMAIL: &str = "user3@gitsecret.io";
const FIRST_FILE: &str = "space file";
const SECOND_FILE: &str = "space file two";

#[test]
fn remove_runs_normally_for_nameless_user() {
    let context = noname_context();

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

struct NonameContext {
    repo: TempRepo,
    _gpg_home: TempDir,
}

fn noname_context() -> NonameContext {
    let gpg_home = TempDir::new();
    import_public_fixture_key(gpg_home.path(), NONAME_USER_EMAIL);

    let repo = TempRepo::new();
    run_success(Command::new("git").arg("init").arg(repo.path()));
    run_success(git_secret(repo.path()).arg("init").current_dir(repo.path()));

    assert_success(
        &git_secret(repo.path())
            .arg("tell")
            .arg("-d")
            .arg(gpg_home.path())
            .arg(NONAME_USER_EMAIL)
            .output()
            .expect("git-secret tell should run"),
    );

    write_file(repo.path(), FIRST_FILE, "somecontent");
    write_file(repo.path(), SECOND_FILE, "somecontent2");
    assert_success(&git_secret_add(repo.path(), &[FIRST_FILE, SECOND_FILE]));
    assert_success(&git_secret_hide(repo.path()));

    NonameContext {
        repo,
        _gpg_home: gpg_home,
    }
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
