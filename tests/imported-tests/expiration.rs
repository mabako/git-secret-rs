use std::fs;
use std::path::Path;
use std::process::{Command, Output};

#[path = "../support/mod.rs"]
mod support;

use support::{
    assert_failure, assert_stdout_contains, assert_success, git_secret, import_public_fixture_key,
    run_success, TempDir, TempRepo,
};

const EXPIRED_USER_EMAIL: &str = "user4@gitsecret.io";
const EXPIRED_USER_UID: &str = "user4 <user4@gitsecret.io>";
const DEFAULT_USER_EMAIL: &str = "user1@gitsecret.io";
const DEFAULT_USER_UID: &str = "user1 <user1@gitsecret.io>";
const FILE_TO_HIDE: &str = "space file";
const FILE_CONTENTS: &str = "hidden content юникод";

#[test]
fn hide_using_expired_key_fails() {
    let context = expiration_context();
    write_file(context.repo.path(), FILE_TO_HIDE, FILE_CONTENTS);
    assert_success(&git_secret_add(context.repo.path(), &[FILE_TO_HIDE]));

    let output = git_secret(context.repo.path())
        .arg("hide")
        .output()
        .expect("git-secret hide should run");

    assert_failure(&output);
}

#[test]
fn whoknows_using_expired_key_succeeds() {
    let context = expiration_context();

    let output = git_secret_whoknows(context.repo.path(), &[]);

    assert_success(&output);
    assert_stdout_contains(&output, EXPIRED_USER_EMAIL);
}

#[test]
fn whoknows_long_prints_expired_user_expiration() {
    let context = expiration_context();

    let output = git_secret_whoknows(context.repo.path(), &["-l"]);

    assert_success(&output);
    assert_stdout_contains(
        &output,
        &format!("{EXPIRED_USER_UID} (expires: 2018-09-23)"),
    );
}

#[test]
fn whoknows_long_prints_normal_and_expired_user_expirations() {
    let context = expiration_context();
    import_public_fixture_key(context.gpg_home.path(), DEFAULT_USER_EMAIL);
    assert_success(&git_secret_tell(
        context.repo.path(),
        context.gpg_home.path(),
        DEFAULT_USER_EMAIL,
    ));

    let output = git_secret_whoknows(context.repo.path(), &["-l"]);

    assert_success(&output);
    assert_stdout_contains(&output, &format!("{DEFAULT_USER_UID} (expires: never)"));
    assert_stdout_contains(
        &output,
        &format!("{EXPIRED_USER_UID} (expires: 2018-09-23)"),
    );
}

struct ExpirationContext {
    repo: TempRepo,
    gpg_home: TempDir,
}

fn expiration_context() -> ExpirationContext {
    let gpg_home = TempDir::new();
    import_public_fixture_key(gpg_home.path(), EXPIRED_USER_EMAIL);

    let repo = TempRepo::new();
    run_success(Command::new("git").arg("init").arg(repo.path()));
    run_success(git_secret(repo.path()).arg("init").current_dir(repo.path()));
    assert_success(&git_secret_tell(
        repo.path(),
        gpg_home.path(),
        EXPIRED_USER_EMAIL,
    ));

    ExpirationContext { repo, gpg_home }
}

fn git_secret_tell(current_dir: &Path, gpg_home: &Path, key: &str) -> Output {
    git_secret(current_dir)
        .arg("tell")
        .arg("-d")
        .arg(gpg_home)
        .arg(key)
        .output()
        .expect("git-secret tell should run")
}

fn git_secret_add(current_dir: &Path, paths: &[&str]) -> Output {
    git_secret(current_dir)
        .arg("add")
        .args(paths)
        .output()
        .expect("git-secret add should run")
}

fn git_secret_whoknows(current_dir: &Path, args: &[&str]) -> Output {
    git_secret(current_dir)
        .arg("whoknows")
        .args(args)
        .output()
        .expect("git-secret whoknows should run")
}

fn write_file(repo: &Path, path: &str, content: &str) {
    fs::write(repo.join(path), content).expect("secret should be written");
}
