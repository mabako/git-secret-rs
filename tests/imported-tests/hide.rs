use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[path = "../support/mod.rs"]
mod support;

use support::{
    assert_failure, assert_stdout_contains, assert_success, git_secret, import_public_fixture_key,
    run_success, TempRepo,
};

const USER1_EMAIL: &str = "user1@gitsecret.io";
const USER2_EMAIL: &str = "user2@gitsecret.io";
const FILE_TO_HIDE: &str = "space file";
const SECOND_FILE: &str = "space file two";
const FILE_CONTENTS: &str = "hidden content юникод";

#[test]
fn hide_runs_normally() {
    let context = hide_context(&[USER1_EMAIL]);

    let output = git_secret_hide(context.repo.path(), &[]);

    assert_success(&output);
    assert_done(&output, 1, 1);
    assert_encrypted_file_exists(context.repo.path(), FILE_TO_HIDE);
    assert_encrypted_file_is_not_plaintext(context.repo.path(), FILE_TO_HIDE);
}

#[test]
fn hide_supports_gpg_armor() {
    let context = hide_context(&[USER1_EMAIL]);

    let output = git_secret(context.repo.path())
        .arg("hide")
        .env("SECRETS_GPG_ARMOR", "1")
        .output()
        .expect("git-secret hide should run");

    assert_success(&output);
    assert_done(&output, 1, 1);
    assert_encrypted_file_exists(context.repo.path(), FILE_TO_HIDE);
    assert!(
        fs::read_to_string(encrypted_path(context.repo.path(), FILE_TO_HIDE))
            .expect("armored secret should be text")
            .starts_with("-----BEGIN PGP MESSAGE-----")
    );
}

#[test]
fn hide_rejects_extra_filename() {
    let context = hide_context(&[USER1_EMAIL]);

    let output = git_secret_hide(context.repo.path(), &["extra_filename"]);

    assert_failure(&output);
}

#[test]
fn hide_rejects_bad_argument() {
    let context = hide_context(&[USER1_EMAIL]);

    let output = git_secret(context.repo.path())
        .arg("hide")
        .arg("-Z")
        .output()
        .expect("git-secret hide should run");

    assert_failure(&output);
}

#[test]
fn hide_runs_with_secrets_verbose() {
    let context = hide_context(&[USER1_EMAIL]);

    let output = git_secret(context.repo.path())
        .arg("hide")
        .env("SECRETS_VERBOSE", "1")
        .output()
        .expect("git-secret hide should run");

    assert_success(&output);
    assert_done(&output, 1, 1);
}

#[cfg(unix)]
#[test]
fn hide_preserves_permissions_with_p() {
    use std::os::unix::fs::PermissionsExt;

    let context = hide_context(&[USER1_EMAIL]);
    let plaintext = context.repo.path().join(FILE_TO_HIDE);
    fs::set_permissions(&plaintext, fs::Permissions::from_mode(0o600))
        .expect("plaintext permissions should be updated");

    let output = git_secret_hide(context.repo.path(), &["-P"]);

    assert_success(&output);
    assert_done(&output, 1, 1);
    let plaintext_mode = fs::metadata(&plaintext)
        .expect("plaintext metadata should be readable")
        .permissions()
        .mode()
        & 0o777;
    let encrypted_mode = fs::metadata(encrypted_path(context.repo.path(), FILE_TO_HIDE))
        .expect("encrypted metadata should be readable")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(encrypted_mode, plaintext_mode);
}

#[test]
fn hide_runs_from_inside_subdirectory() {
    let context = hide_context(&[USER1_EMAIL]);
    let root_dir = context.repo.path().join("test_sub_dir");
    fs::create_dir_all(&root_dir).expect("subdirectory should be created");
    let second_file = "test_sub_dir/second_file.txt";
    write_file(context.repo.path(), second_file, "some content");
    assert_success(&git_secret_add(context.repo.path(), &[second_file]));

    let output = git_secret_hide(&root_dir, &[]);

    assert_success(&output);
    assert_done(&output, 2, 2);
    assert_encrypted_file_exists(context.repo.path(), FILE_TO_HIDE);
    assert_encrypted_file_exists(context.repo.path(), second_file);
}

#[test]
fn hide_fails_when_a_file_is_missing() {
    let context = hide_context(&[USER1_EMAIL]);
    write_file(context.repo.path(), SECOND_FILE, "some content");
    assert_success(&git_secret_add(context.repo.path(), &[SECOND_FILE]));
    fs::remove_file(context.repo.path().join(SECOND_FILE)).expect("second file should be removed");

    let output = git_secret_hide(context.repo.path(), &[]);

    assert_failure(&output);
    assert_stdout_does_not_contain(&output, "git-secret: done. 2 of 2 files are hidden.");
}

#[test]
fn hide_accepts_multiple_files() {
    let context = hide_context(&[USER1_EMAIL]);
    write_file(context.repo.path(), SECOND_FILE, "some content");
    assert_success(&git_secret_add(context.repo.path(), &[SECOND_FILE]));

    let output = git_secret_hide(context.repo.path(), &[]);

    assert_success(&output);
    assert_done(&output, 2, 2);
}

#[test]
fn hide_modified_only_runs_first_time() {
    let context = hide_context(&[USER1_EMAIL]);

    let output = git_secret_hide(context.repo.path(), &["-m"]);

    assert_success(&output);
    assert_done(&output, 1, 1);
    assert_encrypted_file_exists(context.repo.path(), FILE_TO_HIDE);
}

#[test]
fn hide_modified_only_skips_unchanged_file_second_time() {
    let context = hide_context(&[USER1_EMAIL]);
    let mapping = mapping_path(context.repo.path());

    assert_success(&git_secret_hide(context.repo.path(), &["-m"]));
    let original_mapping = fs::read_to_string(&mapping).expect("mapping should be readable");
    let output = git_secret_hide(context.repo.path(), &["-m"]);

    assert_success(&output);
    assert_stdout_contains(&output, "unchanged space file");
    assert_done(&output, 0, 1);
    assert_eq!(
        fs::read_to_string(&mapping).expect("mapping should be readable"),
        original_mapping
    );
    assert_encrypted_file_exists(context.repo.path(), FILE_TO_HIDE);
}

#[test]
fn hide_then_modified_only_skips_unchanged_file() {
    let context = hide_context(&[USER1_EMAIL]);
    let mapping = mapping_path(context.repo.path());

    assert_success(&git_secret_hide(context.repo.path(), &[]));
    let original_mapping = fs::read_to_string(&mapping).expect("mapping should be readable");
    let output = git_secret_hide(context.repo.path(), &["-m"]);

    assert_success(&output);
    assert_done(&output, 0, 1);
    assert_eq!(
        fs::read_to_string(&mapping).expect("mapping should be readable"),
        original_mapping
    );
    assert_encrypted_file_exists(context.repo.path(), FILE_TO_HIDE);
}

#[test]
fn hide_clean_verbose_reports_deleted_encrypted_file() {
    let context = hide_context(&[USER1_EMAIL]);
    assert_success(&git_secret_hide(context.repo.path(), &[]));

    let output = git_secret_hide(context.repo.path(), &["-v", "-c"]);

    assert_success(&output);
    assert!(context.repo.path().join(FILE_TO_HIDE).is_file());
    assert_stdout_contains(&output, "deleted:");
    assert_stdout_contains(&output, &format!("{FILE_TO_HIDE}.secret"));
}

#[test]
fn hide_delete_plaintext_removes_file() {
    let context = hide_context(&[USER1_EMAIL]);

    let output = git_secret_hide(context.repo.path(), &["-d"]);

    assert_success(&output);
    assert!(!context.repo.path().join(FILE_TO_HIDE).exists());
}

#[test]
fn hide_delete_plaintext_verbose_reports_file() {
    let context = hide_context(&[USER1_EMAIL]);

    let output = git_secret_hide(context.repo.path(), &["-v", "-d"]);

    assert_success(&output);
    assert!(!context.repo.path().join(FILE_TO_HIDE).exists());
    assert_stdout_contains(&output, "removing unencrypted files");
    assert_stdout_contains(&output, FILE_TO_HIDE);
}

#[test]
fn hide_delete_plaintext_verbose_reports_files_in_subdirectories() {
    let context = hide_context(&[USER1_EMAIL]);
    let second_file = format!("test_sub_dir/{SECOND_FILE}");
    fs::create_dir_all(context.repo.path().join("test_sub_dir"))
        .expect("subdirectory should be created");
    write_file(context.repo.path(), &second_file, "some content");
    assert_success(&git_secret_add(context.repo.path(), &[&second_file]));

    let output = git_secret_hide(context.repo.path(), &["-v", "-d"]);

    assert_success(&output);
    assert!(!context.repo.path().join(FILE_TO_HIDE).exists());
    assert!(!context.repo.path().join(&second_file).exists());
    assert_stdout_contains(&output, "removing unencrypted files");
    assert_stdout_contains(&output, FILE_TO_HIDE);
    assert_stdout_contains(&output, &second_file);
}

#[test]
fn hide_accepts_multiple_users() {
    let context = hide_context(&[USER1_EMAIL, USER2_EMAIL]);

    let output = git_secret_hide(context.repo.path(), &[]);

    assert_success(&output);
    assert_done(&output, 1, 1);
    assert_encrypted_file_exists(context.repo.path(), FILE_TO_HIDE);
}

struct HideContext {
    repo: TempRepo,
}

fn hide_context(emails: &[&str]) -> HideContext {
    let repo = TempRepo::new();
    run_success(Command::new("git").arg("init").arg(repo.path()));
    run_success(git_secret(repo.path()).arg("init").current_dir(repo.path()));

    let keyring = repo.path().join(".gitsecret").join("keys");
    for email in emails {
        import_public_fixture_key(&keyring, email);
    }

    write_file(repo.path(), FILE_TO_HIDE, FILE_CONTENTS);
    assert_success(&git_secret_add(repo.path(), &[FILE_TO_HIDE]));

    HideContext { repo }
}

fn git_secret_add(current_dir: &Path, paths: &[&str]) -> Output {
    git_secret(current_dir)
        .arg("add")
        .args(paths)
        .output()
        .expect("git-secret add should run")
}

fn git_secret_hide(current_dir: &Path, args: &[&str]) -> Output {
    git_secret(current_dir)
        .arg("hide")
        .args(args)
        .output()
        .expect("git-secret hide should run")
}

fn write_file(repo: &Path, path: &str, content: &str) {
    let path = repo.join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent directory should be created");
    }
    fs::write(path, content).expect("secret should be written");
}

fn encrypted_path(repo: &Path, path: &str) -> PathBuf {
    repo.join(format!("{path}.secret"))
}

fn mapping_path(repo: &Path) -> PathBuf {
    repo.join(".gitsecret/paths/mapping.cfg")
}

fn assert_encrypted_file_exists(repo: &Path, path: &str) {
    let encrypted = encrypted_path(repo, path);
    assert!(encrypted.is_file(), "{} should exist", encrypted.display());
}

fn assert_encrypted_file_is_not_plaintext(repo: &Path, path: &str) {
    let encrypted =
        fs::read(encrypted_path(repo, path)).expect("encrypted file should be readable");
    assert_ne!(encrypted, FILE_CONTENTS.as_bytes());
}

fn assert_done(output: &Output, hidden: usize, total: usize) {
    assert_stdout_contains(
        output,
        &format!("git-secret: done. {hidden} of {total} files are hidden."),
    );
}

fn assert_stdout_does_not_contain(output: &Output, unexpected: &str) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(unexpected),
        "stdout should not contain {unexpected:?}:\n{stdout}"
    );
}
