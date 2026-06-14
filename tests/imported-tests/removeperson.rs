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
const USER1_FINGERPRINT: &str = "CE82DD3AFC167295F9132371D2805A4182E99FF4";
const USER1_UID: &str = "user1 <user1@gitsecret.io>";
const USER2_EMAIL: &str = "user2@gitsecret.io";
const USER2_UID: &str = "user2 <user2@gitsecret.io>";

struct RemovePersonContext {
    repo: TempRepo,
    gpg_home: TempDir,
}

#[test]
fn removeperson_fails_without_arguments() {
    let context = removeperson_context(&[USER1_EMAIL]);

    let output = git_secret(context.repo.path())
        .arg("removeperson")
        .output()
        .expect("git-secret removeperson should run");

    assert_failure(&output);
}

#[test]
fn removeperson_rejects_short_name() {
    let context = removeperson_context(&[USER1_EMAIL]);

    let output = git_secret_removeperson(context.repo.path(), &["user1"]);

    assert_failure(&output);
    assert_whoknows_contains(context.repo.path(), USER1_UID);
}

#[test]
fn removeperson_removes_email() {
    let context = removeperson_context(&[USER1_EMAIL]);

    let output = git_secret_removeperson(context.repo.path(), &[USER1_EMAIL]);

    assert_success(&output);
    assert_stdout_contains(&output, USER1_EMAIL);
    assert_no_recipients(context.repo.path());
}

#[test]
fn removeperson_accepts_multiple_emails() {
    let context = removeperson_context(&[USER1_EMAIL, USER2_EMAIL]);

    let output = git_secret_removeperson(context.repo.path(), &[USER1_EMAIL, USER2_EMAIL]);

    assert_success(&output);
    assert_stdout_contains(&output, USER1_EMAIL);
    assert_stdout_contains(&output, USER2_EMAIL);
    assert_no_recipients(context.repo.path());
}

#[test]
fn removeperson_rejects_bad_argument() {
    let context = removeperson_context(&[USER1_EMAIL]);

    let output = git_secret(context.repo.path())
        .arg("removeperson")
        .arg("-Z")
        .arg(USER1_EMAIL)
        .output()
        .expect("git-secret removeperson should run");

    assert_failure(&output);
}

#[test]
fn killperson_prints_deprecation_error() {
    let context = removeperson_context(&[USER1_EMAIL]);

    let output = git_secret(context.repo.path())
        .arg("killperson")
        .arg(USER1_EMAIL)
        .output()
        .expect("git-secret killperson should run");

    assert_failure(&output);
    assert_stderr_contains(&output, "killperson is deprecated");
    assert_stderr_contains(&output, "git secret removeperson");
}

#[test]
fn removeperson_still_removes_email_after_duplicate_tell_fails() {
    let context = removeperson_context(&[USER1_EMAIL]);

    let duplicate_tell = git_secret_tell(context.repo.path(), context.gpg_home.path(), USER1_EMAIL);
    assert_failure(&duplicate_tell);

    let output = git_secret_removeperson(context.repo.path(), &[USER1_EMAIL]);

    assert_success(&output);
    assert_stdout_contains(&output, USER1_EMAIL);
    assert_no_recipients(context.repo.path());
}

#[test]
fn removeperson_removes_fingerprint() {
    let context = removeperson_context(&[USER1_EMAIL]);

    let output = git_secret_removeperson(context.repo.path(), &[USER1_FINGERPRINT]);

    assert_success(&output);
    assert_stdout_contains(&output, USER1_FINGERPRINT);
    assert_no_recipients(context.repo.path());
}

#[test]
fn removeperson_multiple_emails_preserves_unremoved_recipients() {
    let context = removeperson_context(&[USER1_EMAIL, USER2_EMAIL]);

    let output = git_secret_removeperson(context.repo.path(), &[USER1_EMAIL]);

    assert_success(&output);
    assert_whoknows_does_not_contain(context.repo.path(), USER1_UID);
    assert_whoknows_contains(context.repo.path(), USER2_UID);
}

#[test]
fn removeperson_fails_entirely_when_any_removed_key_has_secret_key() {
    let context = removeperson_context(&[USER1_EMAIL, USER2_EMAIL]);
    let private_keys = context
        .repo
        .path()
        .join(".gitsecret")
        .join("keys")
        .join("private-keys-v1.d");
    fs::create_dir_all(&private_keys).expect("private key directory should be created");
    fs::write(private_keys.join("user1.key"), "private key")
        .expect("private key marker should be written");

    let output = git_secret_removeperson(context.repo.path(), &[USER1_EMAIL, USER2_EMAIL]);

    assert_failure(&output);
    assert_stderr_contains(&output, "gpg --homedir");
    assert_stderr_contains(&output, "--delete-secret-keys");
    assert_stderr_contains(&output, USER1_EMAIL);
    assert_whoknows_contains(context.repo.path(), USER1_UID);
    assert_whoknows_contains(context.repo.path(), USER2_UID);
}

fn removeperson_context(emails: &[&str]) -> RemovePersonContext {
    let gpg_home = TempDir::new();
    for email in emails {
        import_public_fixture_key(gpg_home.path(), email);
    }

    let repo = TempRepo::new();
    run_success(Command::new("git").arg("init").arg(repo.path()));
    run_success(git_secret(repo.path()).arg("init").current_dir(repo.path()));
    for email in emails {
        assert_success(&git_secret_tell(repo.path(), gpg_home.path(), email));
    }

    RemovePersonContext { repo, gpg_home }
}

fn git_secret_tell(current_dir: &Path, gpg_home: &Path, email: &str) -> Output {
    git_secret(current_dir)
        .arg("tell")
        .arg("-d")
        .arg(gpg_home)
        .arg(email)
        .output()
        .expect("git-secret tell should run")
}

fn git_secret_removeperson(current_dir: &Path, keys: &[&str]) -> Output {
    git_secret(current_dir)
        .arg("removeperson")
        .args(keys)
        .output()
        .expect("git-secret removeperson should run")
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
    assert_failure(&output);
    assert_stderr_contains(&output, "no recipients configured");
}
