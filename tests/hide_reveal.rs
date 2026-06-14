use std::fs;
use std::path::PathBuf;
use std::process::Command;

mod support;

use support::{
    import_private_fixture_key, import_private_key, import_public_fixture_key, import_public_key,
    run_success, TempDir, TempRepo,
};

const KEY_PASSPHRASE: &str = "user1pass";

#[test]
fn hide_and_reveal_round_trip_with_supplied_keys() {
    let public_key = override_key_path("GIT_SECRET_TEST_PUBLIC_KEY");
    let private_key = override_key_path("GIT_SECRET_TEST_PRIVATE_KEY");

    let repo = TempRepo::new();
    run_success(Command::new("git").arg("init").arg(repo.path()));
    run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("init")
            .current_dir(repo.path()),
    );

    let keyring = repo.path().join(".gitsecret").join("keys");
    if let Some(public_key) = public_key {
        import_public_key_to_repo_keyring(&keyring, &public_key);
    } else {
        import_public_fixture_key(&keyring, "user1@gitsecret.io");
    }
    let user_gpg_home = TempDir::new();
    if let Some(private_key) = private_key {
        import_private_key_to_user_keyring(user_gpg_home.path(), &private_key);
    } else {
        import_private_fixture_key(user_gpg_home.path(), "user1@gitsecret.io");
    }

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

    let custom_encrypted_path = repo.path().join("secret.txt.enc");
    run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("hide")
            .env("SECRETS_EXTENSION", ".enc")
            .current_dir(repo.path()),
    );
    assert!(
        custom_encrypted_path.is_file(),
        "{} should exist",
        custom_encrypted_path.display()
    );
    let custom_cat_output = run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("cat")
            .arg("-d")
            .arg(user_gpg_home.path())
            .arg("-p")
            .arg(KEY_PASSPHRASE)
            .arg("secret.txt")
            .env("SECRETS_EXTENSION", ".enc")
            .current_dir(repo.path()),
    );
    assert_eq!(
        String::from_utf8_lossy(&custom_cat_output.stdout),
        "the launch code changed"
    );

    let changes_output = run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("changes")
            .arg("-d")
            .arg(user_gpg_home.path())
            .arg("-p")
            .arg(KEY_PASSPHRASE)
            .current_dir(repo.path()),
    );
    let changes_stdout = String::from_utf8_lossy(&changes_output.stdout);
    assert!(
        normalize_slashes(&changes_stdout).contains(&format!(
            "changes in {}:",
            normalize_slashes(&secret_path.display().to_string())
        )),
        "changes should list the checked file:\n{}",
        changes_stdout
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
    let changes_stdout = String::from_utf8_lossy(&changes_output.stdout);
    assert!(
        normalize_slashes(&changes_stdout).contains(&format!(
            "changes in {}:",
            normalize_slashes(&secret_path.display().to_string())
        )),
        "changes should list the checked file:\n{}",
        changes_stdout
    );
    assert!(
        changes_stdout.contains("-the launch code changed"),
        "changes should include removed content:\n{}",
        changes_stdout
    );
    assert!(
        changes_stdout.contains("+the launch code drifted"),
        "changes should include added content:\n{}",
        changes_stdout
    );

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
    let reveal_unchanged = run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("reveal")
            .arg("-d")
            .arg(user_gpg_home.path())
            .arg("-p")
            .arg(KEY_PASSPHRASE)
            .current_dir(repo.path()),
    );
    assert_eq!(
        String::from_utf8_lossy(&reveal_unchanged.stdout).trim(),
        "unchanged secret.txt"
    );
    let reveal_always_decrypt = run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("reveal")
            .arg("-a")
            .arg("-d")
            .arg(user_gpg_home.path())
            .arg("-p")
            .arg(KEY_PASSPHRASE)
            .current_dir(repo.path()),
    );
    assert_eq!(
        String::from_utf8_lossy(&reveal_always_decrypt.stdout).trim(),
        "decrypted secret.txt"
    );
}

fn import_public_key_to_repo_keyring(keyring: &PathBuf, public_key: &PathBuf) {
    import_public_key(keyring, public_key);
}

fn import_private_key_to_user_keyring(keyring: &std::path::Path, private_key: &PathBuf) {
    import_private_key(keyring, private_key, KEY_PASSPHRASE);
}

fn override_key_path(env_name: &str) -> Option<PathBuf> {
    let path = std::env::var_os(env_name).map(PathBuf::from)?;
    assert!(
        path.is_file(),
        "{} fixture does not exist: {}",
        env_name,
        path.display()
    );

    Some(path)
}

fn normalize_slashes(value: &str) -> String {
    value.replace('\\', "/").replace("/private/var/", "/var/")
}
