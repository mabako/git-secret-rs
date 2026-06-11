use std::fs;
use std::path::PathBuf;
use std::process::Command;

mod support;

use support::{fixture_path, run_success, TempDir, TempRepo};

const KEY_PASSPHRASE: &str = "user1pass";
const PASSPHRASE_ENV: &str = "GIT_SECRET_GPG_PASSPHRASE";

#[test]
fn hide_and_reveal_round_trip_with_supplied_keys() {
    let public_key = key_path(
        "GIT_SECRET_TEST_PUBLIC_KEY",
        fixture_path("keys/public.key"),
    );
    let private_key = key_path(
        "GIT_SECRET_TEST_PRIVATE_KEY",
        fixture_path("keys/private.key"),
    );
    assert!(
        public_key.is_file(),
        "public key fixture does not exist: {}",
        public_key.display()
    );
    assert!(
        private_key.is_file(),
        "private key fixture does not exist: {}",
        private_key.display()
    );

    let repo = TempRepo::new("gshr");
    run_success(Command::new("git").arg("init").arg(repo.path()));
    run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("init")
            .current_dir(repo.path()),
    );

    let keyring = repo.path().join(".gitsecret").join("keys");
    import_public_key_to_repo_keyring(&keyring, &public_key);
    let user_gpg_home = TempDir::new("guser");
    import_private_key_to_user_keyring(user_gpg_home.path(), &private_key);

    let secret_path = repo.path().join("secret.txt");
    fs::write(&secret_path, "the launch code is swordfish")
        .expect("plaintext secret should be written");

    run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("add")
            .arg("secret.txt")
            .current_dir(repo.path()),
    );
    run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("hide")
            .arg("--force")
            .current_dir(repo.path()),
    );

    let mapping = fs::read_to_string(repo.path().join(".gitsecret/paths/mapping.cfg"))
        .expect("mapping should be readable");
    let mapping = mapping.trim();
    assert!(mapping.starts_with("secret.txt:"), "{}", mapping);
    assert_eq!(
        mapping.trim_start_matches("secret.txt:").len(),
        64,
        "hide should store the sha256 digest"
    );

    let encrypted_path = repo.path().join("secret.txt.secret");
    assert!(
        encrypted_path.is_file(),
        "{} should exist",
        encrypted_path.display()
    );
    assert_ne!(
        fs::read(&encrypted_path).expect("encrypted file should be readable"),
        b"the launch code is swordfish"
    );

    fs::remove_file(&secret_path).expect("plaintext should be removed before reveal");
    run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("reveal")
            .env("GNUPGHOME", user_gpg_home.path())
            .env(PASSPHRASE_ENV, KEY_PASSPHRASE)
            .current_dir(repo.path()),
    );

    assert_eq!(
        fs::read_to_string(&secret_path).expect("revealed plaintext should be readable"),
        "the launch code is swordfish"
    );
}

fn import_public_key_to_repo_keyring(keyring: &PathBuf, public_key: &PathBuf) {
    run_success(
        Command::new("gpg")
            .arg("--homedir")
            .arg(keyring)
            .arg("--batch")
            .arg("--import")
            .arg(public_key),
    );
}

fn import_private_key_to_user_keyring(keyring: &std::path::Path, private_key: &PathBuf) {
    run_success(
        Command::new("gpg")
            .arg("--homedir")
            .arg(keyring)
            .arg("--batch")
            .arg("--pinentry-mode")
            .arg("loopback")
            .arg("--passphrase")
            .arg(KEY_PASSPHRASE)
            .arg("--import")
            .arg(private_key),
    );
}

fn key_path(env_name: &str, default: PathBuf) -> PathBuf {
    if let Some(path) = std::env::var_os(env_name) {
        return PathBuf::from(path);
    }

    default
}
