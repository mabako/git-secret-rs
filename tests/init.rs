use std::fs;
use std::process::Command;

mod support;

use support::{run_success, TempRepo};

#[test]
fn init_creates_repository_files_with_empty_keyring() {
    let repo = TempRepo::new("git-secret-init");
    run_success(Command::new("git").arg("init").arg(repo.path()));

    let output = run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("init")
            .current_dir(repo.path()),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("created"), "unexpected stdout: {}", stdout);

    let gitsecret = repo.path().join(".gitsecret");
    let keys = gitsecret.join("keys");
    let paths = gitsecret.join("paths");
    let mapping = paths.join("mapping.cfg");
    let key_gitignore = keys.join(".gitignore");

    assert!(gitsecret.is_dir(), "{} should exist", gitsecret.display());
    assert!(keys.is_dir(), "{} should exist", keys.display());
    assert!(paths.is_dir(), "{} should exist", paths.display());
    assert!(mapping.is_file(), "{} should exist", mapping.display());
    assert!(
        key_gitignore.is_file(),
        "{} should exist",
        key_gitignore.display()
    );
    assert!(
        keys.join("pubring.kbx").is_file(),
        "GPG keybox should exist"
    );
    assert!(
        keys.join("trustdb.gpg").is_file(),
        "GPG trust database should exist"
    );

    assert_eq!(
        fs::read_to_string(&mapping).expect("mapping.cfg should be readable"),
        ""
    );

    let key_gitignore_content =
        fs::read_to_string(&key_gitignore).expect("keys/.gitignore should be readable");
    assert!(key_gitignore_content.contains("random_seed"));
    assert!(key_gitignore_content.contains("trustdb.gpg"));
    assert!(key_gitignore_content.contains("private-keys-v1.d/"));

    let keyring = run_success(
        Command::new("gpg")
            .arg("--homedir")
            .arg(&keys)
            .arg("--with-colons")
            .arg("--list-keys"),
    );
    let keyring_stdout = String::from_utf8_lossy(&keyring.stdout);
    assert!(
        !keyring_stdout.lines().any(|line| line.starts_with("pub:")),
        "repo keyring should not contain public keys:\n{}",
        keyring_stdout
    );
    assert!(
        !keyring_stdout.lines().any(|line| line.starts_with("sec:")),
        "repo keyring should not contain secret keys:\n{}",
        keyring_stdout
    );
}
