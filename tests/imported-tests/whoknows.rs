use std::fs;
use std::path::Path;
use std::process::{Command, Output};

#[path = "../support/mod.rs"]
mod support;

use support::{
    assert_failure, assert_stderr_contains, assert_stdout_contains, assert_success, git_secret,
    import_public_fixture_key, run_success, TempDir, TempRepo,
};

const USER1_EMAIL: &str = "user1@gitsecret.io";
const USER1_UID: &str = "user1 <user1@gitsecret.io>";
const USER2_EMAIL: &str = "user2@gitsecret.io";
const USER2_UID: &str = "user2 <user2@gitsecret.io>";

struct WhoKnowsContext {
    repo: TempRepo,
    _gpg_home: TempDir,
}

#[test]
fn whoknows_runs_normally() {
    let context = whoknows_context();

    let output = git_secret_whoknows(context.repo.path(), &[]);

    assert_success(&output);
    assert_stdout_contains(&output, USER1_EMAIL);
    assert_stdout_contains(&output, USER2_EMAIL);
}

#[test]
fn whoknows_rejects_extra_filename() {
    let context = whoknows_context();

    let output = git_secret_whoknows(context.repo.path(), &["extra_filename"]);

    assert_failure(&output);
}

#[test]
fn whoknows_rejects_bad_argument() {
    let context = whoknows_context();

    let output = git_secret_whoknows(context.repo.path(), &["-Z"]);

    assert_failure(&output);
}

#[test]
fn whoknows_long_prints_expiration() {
    let context = whoknows_context();

    let output = git_secret_whoknows(context.repo.path(), &["-l"]);

    assert_success(&output);
    assert_stdout_contains(&output, &format!("{USER1_UID} (expires: never)"));
    assert_stdout_contains(&output, &format!("{USER2_UID} (expires: never)"));
}

#[test]
fn whoknows_runs_from_subfolder() {
    let context = whoknows_context();
    let subdir = context
        .repo
        .path()
        .join("test_dir")
        .join("subfolders")
        .join("case");
    fs::create_dir_all(&subdir).expect("subdirectory should be created");

    let output = git_secret_whoknows(&subdir, &[]);

    assert_success(&output);
    assert_stdout_contains(&output, USER1_EMAIL);
    assert_stdout_contains(&output, USER2_EMAIL);
}

#[test]
fn whoknows_fails_without_users() {
    let context = whoknows_context();

    assert_success(
        &git_secret(context.repo.path())
            .arg("removeperson")
            .arg(USER1_EMAIL)
            .arg(USER2_EMAIL)
            .output()
            .expect("git-secret removeperson should run"),
    );
    let output = git_secret_whoknows(context.repo.path(), &[]);

    assert_failure(&output);
    assert_stderr_contains(&output, "no recipients configured");
}

fn whoknows_context() -> WhoKnowsContext {
    let gpg_home = TempDir::new();
    import_public_fixture_key(gpg_home.path(), USER1_EMAIL);
    import_public_fixture_key(gpg_home.path(), USER2_EMAIL);

    let repo = TempRepo::new();
    run_success(Command::new("git").arg("init").arg(repo.path()));
    run_success(git_secret(repo.path()).arg("init").current_dir(repo.path()));
    assert_success(&git_secret_tell(
        repo.path(),
        gpg_home.path(),
        &[USER1_EMAIL, USER2_EMAIL],
    ));

    WhoKnowsContext {
        repo,
        _gpg_home: gpg_home,
    }
}

fn git_secret_tell(current_dir: &Path, gpg_home: &Path, keys: &[&str]) -> Output {
    git_secret(current_dir)
        .arg("tell")
        .arg("-d")
        .arg(gpg_home)
        .args(keys)
        .output()
        .expect("git-secret tell should run")
}

fn git_secret_whoknows(current_dir: &Path, args: &[&str]) -> Output {
    git_secret(current_dir)
        .arg("whoknows")
        .args(args)
        .output()
        .expect("git-secret whoknows should run")
}
