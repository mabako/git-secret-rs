use std::fs;
use std::path::Path;
use std::process::{Command, Output};

#[path = "../support/mod.rs"]
mod support;

use support::{
    assert_failure, assert_success, git_secret, import_public_fixture_key, run_success, TempDir,
    TempRepo,
};

const USER1_EMAIL: &str = "user1@gitsecret.io";
const USER1_FINGERPRINT: &str = "CE82DD3AFC167295F9132371D2805A4182E99FF4";
const USER1_UID: &str = "user1 <user1@gitsecret.io>";
const USER2_EMAIL: &str = "user2@gitsecret.io";
const USER2_UID: &str = "user2 <user2@gitsecret.io>";
const USER5_EMAIL: &str = "user5@gitsecret.io";
const INVALID_FINGERPRINT: &str = "DEADBEEFDEADBEEFDEADBEEFDEADBEEFDEADBEEF";

struct TellContext {
    repo: TempRepo,
    gpg_home: TempDir,
}

#[test]
fn tell_accepts_verbose_flag() {
    let context = tell_context(&[USER1_EMAIL]);

    let output = git_secret(context.repo.path())
        .arg("tell")
        .arg("-d")
        .arg(context.gpg_home.path())
        .arg("-v")
        .arg(USER1_EMAIL)
        .output()
        .expect("git-secret tell should run");

    assert_success(&output);
    assert_stdout_contains(&output, USER1_EMAIL);
}

#[test]
fn tell_without_verbose_adds_user_without_gpg_import_noise() {
    let context = tell_context(&[USER1_EMAIL]);

    let output = git_secret_tell(context.repo.path(), context.gpg_home.path(), &[USER1_EMAIL]);

    assert_success(&output);
    assert_stdout_contains(&output, USER1_EMAIL);
    assert!(!String::from_utf8_lossy(&output.stdout).contains("imported:"));
}

#[test]
fn tell_rejects_email_substring() {
    let context = tell_context(&[USER1_EMAIL]);

    let output = git_secret_tell(context.repo.path(), context.gpg_home.path(), &["user"]);

    assert_failure(&output);
    assert_no_recipients(context.repo.path());
}

#[test]
fn tell_rejects_same_email_twice() {
    let context = tell_context(&[USER1_EMAIL]);

    assert_success(&git_secret_tell(
        context.repo.path(),
        context.gpg_home.path(),
        &[USER1_EMAIL],
    ));
    let output = git_secret_tell(context.repo.path(), context.gpg_home.path(), &[USER1_EMAIL]);

    assert_failure(&output);
}

#[test]
fn tell_fails_without_users() {
    let context = tell_context(&[USER1_EMAIL]);

    let output = git_secret(context.repo.path())
        .arg("tell")
        .output()
        .expect("git-secret tell should run");

    assert_failure(&output);
}

#[test]
fn tell_reports_no_recipients_after_removing_last_user() {
    let context = tell_context(&[USER1_EMAIL]);
    assert_success(&git_secret_tell(
        context.repo.path(),
        context.gpg_home.path(),
        &[USER1_EMAIL],
    ));

    assert_success(
        &git_secret(context.repo.path())
            .arg("removeperson")
            .arg(USER1_EMAIL)
            .output()
            .expect("git-secret removeperson should run"),
    );

    assert_no_recipients(context.repo.path());
}

#[test]
fn tell_fails_when_repository_keyring_contains_secret_key_file() {
    let context = tell_context(&[USER1_EMAIL]);
    let private_key = context
        .repo
        .path()
        .join(".gitsecret")
        .join("keys")
        .join("secring.gpg");
    fs::write(&private_key, "private key").expect("legacy secret keyring should be written");

    let output = git_secret_tell(context.repo.path(), context.gpg_home.path(), &[USER1_EMAIL]);

    assert_failure(&output);
}

#[test]
fn tell_fails_without_gitsecret_directory() {
    let context = tell_context(&[USER1_EMAIL]);
    fs::remove_dir_all(context.repo.path().join(".gitsecret"))
        .expect(".gitsecret should be removed");

    let output = git_secret_tell(context.repo.path(), context.gpg_home.path(), &[USER1_EMAIL]);

    assert_failure(&output);
}

#[test]
fn tell_rejects_bad_argument() {
    let context = tell_context(&[USER1_EMAIL]);

    let output = git_secret(context.repo.path())
        .arg("tell")
        .arg("-Z")
        .arg("-d")
        .arg(context.gpg_home.path())
        .arg(USER1_EMAIL)
        .output()
        .expect("git-secret tell should run");

    assert_failure(&output);
}

#[test]
fn tell_adds_user_normally() {
    let context = tell_context(&[USER1_EMAIL]);

    let output = git_secret_tell(context.repo.path(), context.gpg_home.path(), &[USER1_EMAIL]);

    assert_success(&output);
    assert_whoknows_contains(context.repo.path(), USER1_UID);
}

#[test]
fn tell_can_use_git_config_email() {
    let context = tell_context(&[USER1_EMAIL]);
    run_success(
        Command::new("git")
            .arg("config")
            .arg("user.email")
            .arg(USER1_EMAIL)
            .current_dir(context.repo.path()),
    );

    let output = git_secret(context.repo.path())
        .arg("tell")
        .arg("-d")
        .arg(context.gpg_home.path())
        .arg("-m")
        .output()
        .expect("git-secret tell should run");

    assert_success(&output);
    assert_whoknows_contains(context.repo.path(), USER1_UID);
}

#[test]
fn tell_rejects_empty_git_config_email() {
    let context = tell_context(&[USER1_EMAIL]);
    run_success(
        Command::new("git")
            .arg("config")
            .arg("user.email")
            .arg("")
            .current_dir(context.repo.path()),
    );

    let output = git_secret(context.repo.path())
        .arg("tell")
        .arg("-d")
        .arg(context.gpg_home.path())
        .arg("-m")
        .output()
        .expect("git-secret tell should run");

    assert_failure(&output);
}

#[test]
fn tell_accepts_multiple_emails() {
    let context = tell_context(&[USER1_EMAIL, USER2_EMAIL]);

    let output = git_secret_tell(
        context.repo.path(),
        context.gpg_home.path(),
        &[USER1_EMAIL, USER2_EMAIL],
    );

    assert_success(&output);
    assert_whoknows_contains(context.repo.path(), USER1_UID);
    assert_whoknows_contains(context.repo.path(), USER2_UID);
}

#[test]
fn tell_rejects_key_without_matching_email() {
    let context = tell_context(&[USER5_EMAIL]);

    let output = git_secret_tell(context.repo.path(), context.gpg_home.path(), &[USER5_EMAIL]);

    assert_failure(&output);
    assert_whoknows_does_not_contain(context.repo.path(), USER5_EMAIL);
}

#[test]
fn tell_rejects_non_email_name() {
    let context = tell_context(&[USER5_EMAIL]);

    let output = git_secret_tell(context.repo.path(), context.gpg_home.path(), &["user5"]);

    assert_failure(&output);
    assert_whoknows_does_not_contain(context.repo.path(), "user5");
}

#[test]
fn tell_works_from_subdirectory() {
    let context = tell_context(&[USER1_EMAIL]);
    let nested_dir = context.repo.path().join("test_dir").join("telling");
    fs::create_dir_all(&nested_dir).expect("nested tell directory should be created");

    let output = git_secret_tell(&nested_dir, context.gpg_home.path(), &[USER1_EMAIL]);

    assert_success(&output);
    assert_whoknows_contains(context.repo.path(), USER1_UID);
}

#[test]
fn tell_accepts_fingerprint() {
    let context = tell_context(&[USER1_EMAIL]);

    let output = git_secret_tell(
        context.repo.path(),
        context.gpg_home.path(),
        &[USER1_FINGERPRINT],
    );

    assert_success(&output);
    assert_whoknows_contains(context.repo.path(), USER1_UID);
}

#[test]
fn tell_rejects_same_fingerprint_twice() {
    let context = tell_context(&[USER1_EMAIL]);
    assert_success(&git_secret_tell(
        context.repo.path(),
        context.gpg_home.path(),
        &[USER1_FINGERPRINT],
    ));

    let output = git_secret_tell(
        context.repo.path(),
        context.gpg_home.path(),
        &[USER1_FINGERPRINT],
    );

    assert_failure(&output);
}

#[test]
fn tell_rejects_invalid_fingerprint() {
    let context = tell_context(&[USER1_EMAIL]);

    let output = git_secret_tell(
        context.repo.path(),
        context.gpg_home.path(),
        &[INVALID_FINGERPRINT],
    );

    assert_failure(&output);
}

fn tell_context(emails: &[&str]) -> TellContext {
    let gpg_home = TempDir::new("imported-tell-gpg");
    for email in emails {
        import_public_fixture_key(gpg_home.path(), email);
    }

    let repo = TempRepo::new("imported-tell");
    run_success(Command::new("git").arg("init").arg(repo.path()));
    run_success(git_secret(repo.path()).arg("init").current_dir(repo.path()));

    TellContext { repo, gpg_home }
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

fn git_secret_whoknows(current_dir: &Path) -> Output {
    git_secret(current_dir)
        .arg("whoknows")
        .output()
        .expect("git-secret whoknows should run")
}

fn assert_whoknows_contains(current_dir: &Path, expected: &str) {
    let output = git_secret_whoknows(current_dir);
    assert_success(&output);
    assert_stdout_contains(&output, expected);
}

fn assert_whoknows_does_not_contain(current_dir: &Path, unexpected: &str) {
    let output = git_secret_whoknows(current_dir);
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(unexpected),
        "whoknows should not contain {unexpected:?}:\n{stdout}"
    );
}

fn assert_no_recipients(current_dir: &Path) {
    let output = git_secret_whoknows(current_dir);
    assert_success(&output);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "no recipients configured"
    );
}

fn assert_stdout_contains(output: &Output, expected: &str) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(expected),
        "stdout should contain {expected:?}:\n{stdout}"
    );
}
