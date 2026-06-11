use std::fs;
use std::path::PathBuf;
use std::process::Command;

mod support;

use support::{fixture_path, gpg_command, run_success, TempDir, TempRepo};

const KEY_PASSPHRASE: &str = "user1pass";

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
            .current_dir(repo.path()),
    );
    run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("hide")
            .current_dir(repo.path()),
    );
    let hide_modified_only = run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("hide")
            .arg("-m")
            .current_dir(repo.path()),
    );
    assert!(
        String::from_utf8_lossy(&hide_modified_only.stdout).contains("unchanged secret.txt"),
        "hide -m should skip unchanged files:\n{}",
        String::from_utf8_lossy(&hide_modified_only.stdout)
    );

    fs::write(&secret_path, "the launch code changed").expect("plaintext secret should be updated");
    run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("hide")
            .arg("-m")
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
        b"the launch code changed"
    );

    run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("hide")
            .arg("-c")
            .env("SECRETS_GPG_ARMOR", "1")
            .current_dir(repo.path()),
    );
    let armored = fs::read_to_string(&encrypted_path).expect("armored secret should be text");
    assert!(
        armored.starts_with("-----BEGIN PGP MESSAGE-----"),
        "SECRETS_GPG_ARMOR=1 should write armored encrypted output:\n{}",
        armored
    );

    let cat_output = run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("cat")
            .arg("-d")
            .arg(user_gpg_home.path())
            .arg("-p")
            .arg(KEY_PASSPHRASE)
            .arg("secret.txt")
            .current_dir(repo.path()),
    );
    assert_eq!(
        String::from_utf8_lossy(&cat_output.stdout),
        "the launch code changed"
    );
    assert_eq!(String::from_utf8_lossy(&cat_output.stderr), "");
    let textconv_output = run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("textconv")
            .arg("-d")
            .arg(user_gpg_home.path())
            .arg("-p")
            .arg(KEY_PASSPHRASE)
            .arg(&encrypted_path)
            .current_dir(repo.path()),
    );
    assert_eq!(
        String::from_utf8_lossy(&textconv_output.stdout),
        "the launch code changed"
    );
    assert_eq!(String::from_utf8_lossy(&textconv_output.stderr), "");

    let changes_output = run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("changes")
            .arg("-d")
            .arg(user_gpg_home.path())
            .arg("-p")
            .arg(KEY_PASSPHRASE)
            .current_dir(repo.path()),
    );
    assert_eq!(
        String::from_utf8_lossy(&changes_output.stdout).trim(),
        "no changes"
    );

    fs::write(&secret_path, "the launch code drifted").expect("plaintext secret should be updated");
    let changes_output = run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("changes")
            .arg("-d")
            .arg(user_gpg_home.path())
            .arg("-p")
            .arg(KEY_PASSPHRASE)
            .current_dir(repo.path()),
    );
    assert_eq!(
        String::from_utf8_lossy(&changes_output.stdout).trim(),
        "modified\tsecret.txt"
    );

    let cat_help = run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("cat")
            .arg("-h"),
    );
    let cat_help = String::from_utf8_lossy(&cat_help.stdout);
    assert!(cat_help.contains("Usage:"));
    assert!(cat_help.contains("-d"));
    assert!(cat_help.contains("-p"));
    assert!(cat_help.contains("--help"));
    let changes_help = run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("changes")
            .arg("-h"),
    );
    let changes_help = String::from_utf8_lossy(&changes_help.stdout);
    assert!(changes_help.contains("Usage:"));
    assert!(changes_help.contains("-d"));
    assert!(changes_help.contains("-p"));
    assert!(changes_help.contains("--help"));

    fs::remove_file(&secret_path).expect("plaintext should be removed before reveal");
    run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("hide")
            .arg("-F")
            .current_dir(repo.path()),
    );
    run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("reveal")
            .arg("-F")
            .arg("missing.txt")
            .current_dir(repo.path()),
    );
    let reveal_output = run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("reveal")
            .arg("-d")
            .arg(user_gpg_home.path())
            .arg("-p")
            .arg(KEY_PASSPHRASE)
            .env("SECRETS_VERBOSE", "1")
            .current_dir(repo.path()),
    );
    assert!(
        String::from_utf8_lossy(&reveal_output.stdout).contains("decrypted secret.txt from"),
        "SECRETS_VERBOSE should enable verbose reveal output:\n{}",
        String::from_utf8_lossy(&reveal_output.stdout)
    );

    assert_eq!(
        fs::read_to_string(&secret_path).expect("revealed plaintext should be readable"),
        "the launch code changed"
    );

    let hide_help = run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("hide")
            .arg("-h"),
    );
    let hide_help = String::from_utf8_lossy(&hide_help.stdout);
    assert!(hide_help.contains("Usage:"));
    assert!(hide_help.contains("-c"));
    assert!(hide_help.contains("-F"));
    assert!(hide_help.contains("-P"));
    assert!(hide_help.contains("-d"));
    assert!(hide_help.contains("-m"));
    assert!(hide_help.contains("--help"));

    let reveal_help = run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("reveal")
            .arg("-h"),
    );
    let reveal_help = String::from_utf8_lossy(&reveal_help.stdout);
    assert!(reveal_help.contains("Usage:"));
    assert!(reveal_help.contains("-f"));
    assert!(reveal_help.contains("-F"));
    assert!(reveal_help.contains("-d"));
    assert!(reveal_help.contains("-v"));
    assert!(reveal_help.contains("-p"));
    assert!(reveal_help.contains("-P"));
    assert!(reveal_help.contains("--help"));
}

fn import_public_key_to_repo_keyring(keyring: &PathBuf, public_key: &PathBuf) {
    run_success(
        gpg_command()
            .arg("--homedir")
            .arg(keyring)
            .arg("--batch")
            .arg("--import")
            .arg(public_key),
    );
}

fn import_private_key_to_user_keyring(keyring: &std::path::Path, private_key: &PathBuf) {
    run_success(
        gpg_command()
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
