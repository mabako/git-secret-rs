use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[path = "../support/mod.rs"]
mod support;

use support::{
    assert_failure, assert_success, fixture_key_passphrase, git_secret, import_private_fixture_key,
    import_public_fixture_key, run_success, TempDir, TempRepo,
};

const USER1_EMAIL: &str = "user1@gitsecret.io";
const USER2_EMAIL: &str = "user2@gitsecret.io";
const ATTACKER_EMAIL: &str = "attacker1@gitsecret.io";
const FILE_TO_HIDE: &str = "space file";
const FILE_CONTENTS: &str = "hidden content юникод";

struct RevealContext {
    repo: TempRepo,
    gpg_home: TempDir,
}

#[test]
fn reveal_with_password_argument_restores_plaintext() {
    let context = reveal_context(&[USER1_EMAIL], &[USER1_EMAIL]);
    let backup = context.repo.path().join(format!("{FILE_TO_HIDE}2"));
    fs::copy(context.repo.path().join(FILE_TO_HIDE), &backup).expect("backup should be written");
    fs::remove_file(context.repo.path().join(FILE_TO_HIDE)).expect("plaintext should be removed");

    let output = git_secret_reveal(context.repo.path(), context.gpg_home.path(), &[]);

    assert_success(&output);
    assert_file_eq(context.repo.path().join(FILE_TO_HIDE), backup);
}

#[test]
fn reveal_rejects_bad_argument() {
    let context = reveal_context(&[USER1_EMAIL], &[USER1_EMAIL]);

    let output = git_secret(context.repo.path())
        .arg("reveal")
        .arg("-Z")
        .arg("-d")
        .arg(context.gpg_home.path())
        .arg("-p")
        .arg(fixture_key_passphrase(USER1_EMAIL))
        .output()
        .expect("git-secret reveal should run");

    assert_failure(&output);
}

#[test]
fn reveal_rejects_secret_version_of_file() {
    let context = reveal_context(&[USER1_EMAIL], &[USER1_EMAIL]);

    let output = git_secret_reveal(
        context.repo.path(),
        context.gpg_home.path(),
        &[&format!("{FILE_TO_HIDE}.secret")],
    );

    assert_failure(&output);
}

#[test]
fn reveal_rejects_nonexistent_file() {
    let context = reveal_context(&[USER1_EMAIL], &[USER1_EMAIL]);

    let output = git_secret_reveal(
        context.repo.path(),
        context.gpg_home.path(),
        &["DOES-NOT-EXIST"],
    );

    assert_failure(&output);
}

#[test]
fn reveal_with_f_restores_missing_plaintext() {
    let context = reveal_context(&[USER1_EMAIL], &[USER1_EMAIL]);
    fs::remove_file(context.repo.path().join(FILE_TO_HIDE)).expect("plaintext should be removed");

    let output = git_secret(context.repo.path())
        .arg("reveal")
        .arg("-f")
        .arg("-d")
        .arg(context.gpg_home.path())
        .arg("-p")
        .arg(fixture_key_passphrase(USER1_EMAIL))
        .output()
        .expect("git-secret reveal should run");

    assert_success(&output);
    assert_plaintext_exists(context.repo.path());
}

#[test]
fn reveal_binary_with_secrets_gpg_armor_restores_plaintext() {
    let context = reveal_context(&[USER1_EMAIL], &[USER1_EMAIL]);
    fs::remove_file(context.repo.path().join(FILE_TO_HIDE)).expect("plaintext should be removed");

    let output = git_secret(context.repo.path())
        .arg("reveal")
        .arg("-d")
        .arg(context.gpg_home.path())
        .arg("-p")
        .arg(fixture_key_passphrase(USER1_EMAIL))
        .env("SECRETS_GPG_ARMOR", "1")
        .output()
        .expect("git-secret reveal should run");

    assert_success(&output);
    assert_plaintext_exists(context.repo.path());
}

#[test]
fn reveal_armored_with_secrets_gpg_armor_enabled_restores_plaintext() {
    let context = reveal_context(&[USER1_EMAIL], &[USER1_EMAIL]);
    rehides_with_armor(&context);
    fs::remove_file(context.repo.path().join(FILE_TO_HIDE)).expect("plaintext should be removed");

    let output = git_secret(context.repo.path())
        .arg("reveal")
        .arg("-d")
        .arg(context.gpg_home.path())
        .arg("-p")
        .arg(fixture_key_passphrase(USER1_EMAIL))
        .env("SECRETS_GPG_ARMOR", "1")
        .output()
        .expect("git-secret reveal should run");

    assert_success(&output);
    assert_plaintext_exists(context.repo.path());
}

#[test]
fn reveal_armored_with_secrets_gpg_armor_disabled_restores_plaintext() {
    let context = reveal_context(&[USER1_EMAIL], &[USER1_EMAIL]);
    rehides_with_armor(&context);
    fs::remove_file(context.repo.path().join(FILE_TO_HIDE)).expect("plaintext should be removed");

    let output = git_secret(context.repo.path())
        .arg("reveal")
        .arg("-d")
        .arg(context.gpg_home.path())
        .arg("-p")
        .arg(fixture_key_passphrase(USER1_EMAIL))
        .env("SECRETS_GPG_ARMOR", "0")
        .output()
        .expect("git-secret reveal should run");

    assert_success(&output);
    assert_plaintext_exists(context.repo.path());
}

#[test]
fn reveal_with_v_restores_plaintext() {
    let context = reveal_context(&[USER1_EMAIL], &[USER1_EMAIL]);
    fs::remove_file(context.repo.path().join(FILE_TO_HIDE)).expect("plaintext should be removed");

    let output = git_secret(context.repo.path())
        .arg("reveal")
        .arg("-v")
        .arg("-d")
        .arg(context.gpg_home.path())
        .arg("-p")
        .arg(fixture_key_passphrase(USER1_EMAIL))
        .output()
        .expect("git-secret reveal should run");

    assert_success(&output);
    assert_plaintext_exists(context.repo.path());
    assert_stdout_contains(&output, "decrypted");
}

#[cfg(unix)]
#[test]
fn reveal_with_p_preserves_encrypted_file_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let context = reveal_context(&[USER1_EMAIL], &[USER1_EMAIL]);
    fs::remove_file(context.repo.path().join(FILE_TO_HIDE)).expect("plaintext should be removed");
    let encrypted = encrypted_path(context.repo.path(), FILE_TO_HIDE);
    fs::set_permissions(&encrypted, fs::Permissions::from_mode(0o600))
        .expect("encrypted permissions should be updated");

    let output = git_secret(context.repo.path())
        .arg("reveal")
        .arg("-P")
        .arg("-d")
        .arg(context.gpg_home.path())
        .arg("-p")
        .arg(fixture_key_passphrase(USER1_EMAIL))
        .output()
        .expect("git-secret reveal should run");

    assert_success(&output);
    let plaintext_mode = fs::metadata(context.repo.path().join(FILE_TO_HIDE))
        .expect("plaintext metadata should be readable")
        .permissions()
        .mode()
        & 0o777;
    let encrypted_mode = fs::metadata(encrypted)
        .expect("encrypted metadata should be readable")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(plaintext_mode, encrypted_mode);
}

#[test]
fn reveal_with_wrong_password_fails_without_creating_plaintext() {
    let context = reveal_context(&[USER1_EMAIL], &[USER1_EMAIL]);
    fs::remove_file(context.repo.path().join(FILE_TO_HIDE)).expect("plaintext should be removed");

    let output = git_secret(context.repo.path())
        .arg("reveal")
        .arg("-d")
        .arg(context.gpg_home.path())
        .arg("-p")
        .arg("WRONG")
        .output()
        .expect("git-secret reveal should run");

    assert_failure(&output);
    assert_plaintext_does_not_exist(context.repo.path());
}

#[test]
fn reveal_for_attacker_fails_without_creating_plaintext() {
    let context = reveal_context(&[USER1_EMAIL], &[ATTACKER_EMAIL]);
    fs::remove_file(context.repo.path().join(FILE_TO_HIDE)).expect("plaintext should be removed");

    let output = git_secret(context.repo.path())
        .arg("reveal")
        .arg("-d")
        .arg(context.gpg_home.path())
        .arg("-p")
        .arg(fixture_key_passphrase(ATTACKER_EMAIL))
        .output()
        .expect("git-secret reveal should run");

    assert_failure(&output);
    assert_plaintext_does_not_exist(context.repo.path());
}

#[test]
fn reveal_for_attacker_with_f_continues_without_creating_plaintext() {
    let context = reveal_context(&[USER1_EMAIL], &[ATTACKER_EMAIL]);
    fs::remove_file(context.repo.path().join(FILE_TO_HIDE)).expect("plaintext should be removed");

    let output = git_secret(context.repo.path())
        .arg("reveal")
        .arg("-F")
        .arg("-d")
        .arg(context.gpg_home.path())
        .arg("-p")
        .arg(fixture_key_passphrase(ATTACKER_EMAIL))
        .output()
        .expect("git-secret reveal should run");

    assert_success(&output);
    assert_plaintext_does_not_exist(context.repo.path());
}

#[test]
fn reveal_for_multiple_users_works_with_second_private_key_only() {
    let context = reveal_context(&[USER1_EMAIL, USER2_EMAIL], &[USER2_EMAIL]);
    assert_success(&git_secret_hide(context.repo.path()));
    fs::remove_file(context.repo.path().join(FILE_TO_HIDE)).expect("plaintext should be removed");

    let output = git_secret(context.repo.path())
        .arg("reveal")
        .arg("-d")
        .arg(context.gpg_home.path())
        .arg("-p")
        .arg(fixture_key_passphrase(USER2_EMAIL))
        .output()
        .expect("git-secret reveal should run");

    assert_success(&output);
    assert_plaintext_exists(context.repo.path());
}

#[test]
fn reveal_for_multiple_users_works_with_default_private_key() {
    let context = reveal_context(&[USER1_EMAIL, USER2_EMAIL], &[USER1_EMAIL, USER2_EMAIL]);
    assert_success(&git_secret_hide(context.repo.path()));
    fs::remove_file(context.repo.path().join(FILE_TO_HIDE)).expect("plaintext should be removed");

    let output = git_secret_reveal(context.repo.path(), context.gpg_home.path(), &[]);

    assert_success(&output);
    assert_plaintext_exists(context.repo.path());
}

#[test]
fn reveal_with_secrets_pinentry_loopback_restores_plaintext() {
    let context = reveal_context(&[USER1_EMAIL], &[USER1_EMAIL]);
    fs::remove_file(context.repo.path().join(FILE_TO_HIDE)).expect("plaintext should be removed");

    let output = git_secret(context.repo.path())
        .arg("reveal")
        .arg("-d")
        .arg(context.gpg_home.path())
        .arg("-p")
        .arg(fixture_key_passphrase(USER1_EMAIL))
        .env("SECRETS_PINENTRY", "loopback")
        .output()
        .expect("git-secret reveal should run");

    assert_success(&output);
    assert_plaintext_exists(context.repo.path());
}

#[test]
fn reveal_with_secrets_pinentry_error_fails() {
    let context = reveal_context(&[USER1_EMAIL], &[USER1_EMAIL]);
    fs::remove_file(context.repo.path().join(FILE_TO_HIDE)).expect("plaintext should be removed");

    let output = git_secret(context.repo.path())
        .arg("reveal")
        .arg("-d")
        .arg(context.gpg_home.path())
        .arg("-p")
        .arg(fixture_key_passphrase(USER1_EMAIL))
        .env("SECRETS_PINENTRY", "error")
        .output()
        .expect("git-secret reveal should run");

    assert_failure(&output);
}

#[test]
fn reveal_with_named_file_from_subdir() {
    let context = reveal_context(&[USER1_EMAIL], &[USER1_EMAIL]);
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
    assert_success(&git_secret_hide(&subdir));
    fs::remove_file(subdir.join("new_filename.txt")).expect("subdir plaintext should be removed");

    let output = git_secret_reveal(&subdir, context.gpg_home.path(), &["new_filename.txt"]);

    assert_success(&output);
    assert_eq!(
        fs::read_to_string(subdir.join("new_filename.txt"))
            .expect("revealed subdir file should be readable"),
        "content2"
    );
}

#[test]
fn reveal_all_files_from_subdir() {
    let context = reveal_context(&[USER1_EMAIL], &[USER1_EMAIL]);
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
    assert_success(&git_secret_hide(&subdir));
    fs::remove_file(context.repo.path().join(FILE_TO_HIDE))
        .expect("root plaintext should be removed");
    fs::remove_file(subdir.join("new_filename.txt")).expect("subdir plaintext should be removed");

    let output = git_secret_reveal(&subdir, context.gpg_home.path(), &[]);

    assert_success(&output);
    assert_plaintext_exists(context.repo.path());
    assert_eq!(
        fs::read_to_string(subdir.join("new_filename.txt"))
            .expect("revealed subdir file should be readable"),
        "content2"
    );
}

fn reveal_context(recipient_emails: &[&str], private_key_emails: &[&str]) -> RevealContext {
    let repo = TempRepo::new();
    run_success(Command::new("git").arg("init").arg(repo.path()));
    run_success(git_secret(repo.path()).arg("init").current_dir(repo.path()));

    let keyring = repo.path().join(".gitsecret").join("keys");
    for email in recipient_emails {
        import_public_fixture_key(&keyring, email);
    }

    let gpg_home = TempDir::new();
    for email in private_key_emails {
        import_private_fixture_key(gpg_home.path(), email);
    }

    fs::write(repo.path().join(FILE_TO_HIDE), FILE_CONTENTS).expect("secret should be written");
    assert_success(
        &git_secret(repo.path())
            .arg("add")
            .arg(FILE_TO_HIDE)
            .output()
            .expect("git-secret add should run"),
    );
    assert_success(&git_secret_hide(repo.path()));

    RevealContext { repo, gpg_home }
}

fn git_secret_hide(current_dir: &Path) -> Output {
    git_secret(current_dir)
        .arg("hide")
        .output()
        .expect("git-secret hide should run")
}

fn git_secret_reveal(current_dir: &Path, gpg_home: &Path, paths: &[&str]) -> Output {
    git_secret(current_dir)
        .arg("reveal")
        .arg("-d")
        .arg(gpg_home)
        .arg("-p")
        .arg(fixture_key_passphrase(USER1_EMAIL))
        .args(paths)
        .output()
        .expect("git-secret reveal should run")
}

fn rehides_with_armor(context: &RevealContext) {
    assert_success(
        &git_secret(context.repo.path())
            .arg("clean")
            .output()
            .expect("git-secret clean should run"),
    );
    assert_success(
        &git_secret(context.repo.path())
            .arg("hide")
            .env("SECRETS_GPG_ARMOR", "1")
            .output()
            .expect("git-secret hide should run"),
    );
}

#[cfg(unix)]
fn encrypted_path(repo: &Path, path: &str) -> PathBuf {
    repo.join(format!("{path}.secret"))
}

fn assert_file_eq(left: PathBuf, right: PathBuf) {
    assert_eq!(
        fs::read(&left).expect("left file should be readable"),
        fs::read(&right).expect("right file should be readable")
    );
}

fn assert_plaintext_exists(repo: &Path) {
    assert!(repo.join(FILE_TO_HIDE).is_file());
}

fn assert_plaintext_does_not_exist(repo: &Path) {
    assert!(!repo.join(FILE_TO_HIDE).exists());
}

fn assert_stdout_contains(output: &Output, expected: &str) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(expected),
        "stdout should contain {expected:?}:\n{stdout}"
    );
}
