use std::fs;
use std::path::Path;
use std::process::{Command, Output};

#[path = "../support/mod.rs"]
mod support;

use support::{
    assert_failure, assert_success, git_secret, import_private_fixture_key,
    import_public_fixture_key, run_success, TempDir, TempRepo,
};

const USER1_EMAIL: &str = "user1@gitsecret.io";
const FILE_TO_HIDE: &str = "space file";
const FILE_CONTENTS: &str = "hidden content юникод";

struct CatContext {
    repo: TempRepo,
    gpg_home: TempDir,
}

#[test]
fn cat_with_password_argument_prints_plaintext() {
    let context = cat_context();

    let output = git_secret_cat(
        context.repo.path(),
        context.gpg_home.path(),
        &[FILE_TO_HIDE],
    );

    assert_success(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), FILE_CONTENTS);
}

#[test]
fn cat_with_password_argument_and_secrets_verbose_prints_plaintext() {
    let context = cat_context();

    let output = git_secret(context.repo.path())
        .arg("cat")
        .arg("-d")
        .arg(context.gpg_home.path())
        .arg("-p")
        .arg(fixture_passphrase())
        .arg(FILE_TO_HIDE)
        .env("SECRETS_VERBOSE", "1")
        .output()
        .expect("git-secret cat should run");

    assert_success(&output);
    assert_stdout_contains(&output, FILE_CONTENTS);
}

#[test]
fn cat_rejects_bad_filename() {
    let context = cat_context();

    let output = git_secret_cat(
        context.repo.path(),
        context.gpg_home.path(),
        &["NO_SUCH_FILE"],
    );

    assert_failure(&output);
}

#[test]
fn cat_rejects_bad_argument() {
    let context = cat_context();

    let output = git_secret(context.repo.path())
        .arg("cat")
        .arg("-Z")
        .arg("-d")
        .arg(context.gpg_home.path())
        .arg("-p")
        .arg(fixture_passphrase())
        .arg(FILE_TO_HIDE)
        .output()
        .expect("git-secret cat should run");

    assert_failure(&output);
}

#[test]
fn cat_runs_from_subdirectory() {
    let context = cat_context();
    let subdir = context.repo.path().join("subdir");
    fs::create_dir(&subdir).expect("subdir should be created");
    fs::write(subdir.join("new_filename.txt"), "content2").expect("subdir file should be written");

    assert_success(
        &git_secret(&subdir)
            .arg("add")
            .arg("new_filename.txt")
            .output()
            .expect("git-secret add should run"),
    );
    assert_success(
        &git_secret(&subdir)
            .arg("hide")
            .output()
            .expect("git-secret hide should run"),
    );

    let output = git_secret_cat(&subdir, context.gpg_home.path(), &["new_filename.txt"]);

    assert_success(&output);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "content2");
}

fn cat_context() -> CatContext {
    let repo = TempRepo::new();
    run_success(Command::new("git").arg("init").arg(repo.path()));
    run_success(git_secret(repo.path()).arg("init").current_dir(repo.path()));

    import_public_fixture_key(&repo.path().join(".gitsecret").join("keys"), USER1_EMAIL);

    let gpg_home = TempDir::new();
    import_private_fixture_key(gpg_home.path(), USER1_EMAIL);

    fs::write(repo.path().join(FILE_TO_HIDE), FILE_CONTENTS).expect("secret should be written");
    assert_success(
        &git_secret(repo.path())
            .arg("add")
            .arg(FILE_TO_HIDE)
            .output()
            .expect("git-secret add should run"),
    );
    assert_success(
        &git_secret(repo.path())
            .arg("hide")
            .output()
            .expect("git-secret hide should run"),
    );

    CatContext { repo, gpg_home }
}

fn git_secret_cat(current_dir: &Path, gpg_home: &Path, paths: &[&str]) -> Output {
    git_secret(current_dir)
        .arg("cat")
        .arg("-d")
        .arg(gpg_home)
        .arg("-p")
        .arg(fixture_passphrase())
        .args(paths)
        .output()
        .expect("git-secret cat should run")
}

fn fixture_passphrase() -> &'static str {
    "user1pass"
}

fn assert_stdout_contains(output: &Output, expected: &str) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(expected),
        "stdout should contain {expected:?}:\n{stdout}"
    );
}
