use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[path = "../support/mod.rs"]
mod support;

use support::{
    assert_success, fixture_key_passphrase, git_secret, import_private_fixture_key,
    import_public_fixture_key, run_success, TempDir, TempRepo,
};

const USER1_EMAIL: &str = "user1@gitsecret.io";
const FILE_TO_HIDE: &str = "space file";
const FILE_CONTENTS: &str = "hidden content юникод";
const CUSTOM_EXTENSION: &str = ".new_secret";

#[test]
fn reveal_with_different_file_extension_restores_plaintext() {
    let context = reveal_filename_context();
    let backup = context.repo.path().join(format!("{FILE_TO_HIDE}2"));
    fs::copy(context.repo.path().join(FILE_TO_HIDE), &backup).expect("backup should be written");
    fs::remove_file(context.repo.path().join(FILE_TO_HIDE)).expect("plaintext should be removed");

    let output = git_secret(context.repo.path())
        .arg("reveal")
        .arg("-d")
        .arg(context.gpg_home.path())
        .arg("-p")
        .arg(fixture_key_passphrase(USER1_EMAIL))
        .env("SECRETS_EXTENSION", CUSTOM_EXTENSION)
        .output()
        .expect("git-secret reveal should run");

    assert_success(&output);
    assert!(context.repo.path().join(FILE_TO_HIDE).is_file());
    assert_file_eq(context.repo.path().join(FILE_TO_HIDE), backup);
}

struct RevealFilenameContext {
    repo: TempRepo,
    gpg_home: TempDir,
}

fn reveal_filename_context() -> RevealFilenameContext {
    let repo = TempRepo::new("irf");
    run_success(Command::new("git").arg("init").arg(repo.path()));
    run_success(git_secret(repo.path()).arg("init").current_dir(repo.path()));

    import_public_fixture_key(&repo.path().join(".gitsecret").join("keys"), USER1_EMAIL);

    let gpg_home = TempDir::new("irf-gpg");
    import_private_fixture_key(gpg_home.path(), USER1_EMAIL);

    fs::write(repo.path().join(FILE_TO_HIDE), FILE_CONTENTS).expect("secret should be written");
    assert_success(
        &git_secret(repo.path())
            .arg("add")
            .arg(FILE_TO_HIDE)
            .output()
            .expect("git-secret add should run"),
    );
    assert_success(&git_secret_hide_with_custom_extension(repo.path()));

    RevealFilenameContext { repo, gpg_home }
}

fn git_secret_hide_with_custom_extension(current_dir: &Path) -> Output {
    git_secret(current_dir)
        .arg("hide")
        .env("SECRETS_EXTENSION", CUSTOM_EXTENSION)
        .output()
        .expect("git-secret hide should run")
}

fn assert_file_eq(left: PathBuf, right: PathBuf) {
    assert_eq!(
        fs::read(&left).expect("left file should be readable"),
        fs::read(&right).expect("right file should be readable")
    );
}
