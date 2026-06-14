use std::fs;
use std::process::Command;

mod support;

use support::{gpg_arg_path, gpg_command, run_success, TempRepo};

#[test]
fn init_creates_repository_files_with_empty_keyring() {
    let repo = TempRepo::new();
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
    let root_gitignore = repo.path().join(".gitignore");
    let root_gitattributes = repo.path().join(".gitattributes");

    assert!(gitsecret.is_dir(), "{} should exist", gitsecret.display());
    assert!(keys.is_dir(), "{} should exist", keys.display());
    assert!(paths.is_dir(), "{} should exist", paths.display());
    assert!(mapping.is_file(), "{} should exist", mapping.display());
    assert!(
        !keys.join(".gitignore").exists(),
        ".gitsecret/keys/.gitignore should not be created"
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

    assert_eq!(
        fs::read_to_string(&root_gitignore).expect("root .gitignore should be readable"),
        ".gitsecret/keys/random_seed\n.gitsecret/keys/*.lock\n!*.secret\n"
    );
    assert_eq!(
        fs::read_to_string(&root_gitattributes).expect("root .gitattributes should be readable"),
        "*.secret diff=git-secret\n"
    );
    let textconv = run_success(
        Command::new("git")
            .arg("config")
            .arg("--get")
            .arg("diff.git-secret.textconv")
            .current_dir(repo.path()),
    );
    assert_eq!(
        String::from_utf8_lossy(&textconv.stdout).trim(),
        "git-secret textconv"
    );

    let keyring = run_success(
        gpg_command()
            .arg("--homedir")
            .arg(gpg_arg_path(&keys))
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

#[test]
fn init_uses_custom_secrets_dir() {
    let repo = TempRepo::new();
    run_success(Command::new("git").arg("init").arg(repo.path()));

    run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("init")
            .env("SECRETS_DIR", ".secrets")
            .current_dir(repo.path()),
    );

    let custom_dir = repo.path().join(".secrets");
    let keys = custom_dir.join("keys");
    let paths = custom_dir.join("paths");
    let mapping = paths.join("mapping.cfg");

    assert!(custom_dir.is_dir(), "{} should exist", custom_dir.display());
    assert!(keys.is_dir(), "{} should exist", keys.display());
    assert!(paths.is_dir(), "{} should exist", paths.display());
    assert!(mapping.is_file(), "{} should exist", mapping.display());
    assert!(
        !repo.path().join(".gitsecret").exists(),
        "default storage directory should not be created"
    );

    assert_eq!(
        fs::read_to_string(&mapping).expect("mapping.cfg should be readable"),
        ""
    );

    let gitignore =
        fs::read_to_string(repo.path().join(".gitignore")).expect(".gitignore should be readable");
    assert!(gitignore.contains(".secrets/keys/random_seed\n"));
    assert!(gitignore.contains(".secrets/keys/*.lock\n"));
    assert!(gitignore.contains("!*.secret\n"));
}

#[test]
fn init_adds_default_gitignore_entries_without_duplicates() {
    let repo = TempRepo::new();
    run_success(Command::new("git").arg("init").arg(repo.path()));
    fs::write(
        repo.path().join(".gitignore"),
        "target\n!*.secret\nexisting-without-newline",
    )
    .expect("existing .gitignore should be written");

    run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("init")
            .current_dir(repo.path()),
    );
    run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("init")
            .current_dir(repo.path()),
    );

    let gitignore =
        fs::read_to_string(repo.path().join(".gitignore")).expect(".gitignore should be readable");
    assert!(gitignore.contains("target\n"));
    assert!(gitignore.contains("existing-without-newline\n"));
    assert_eq!(
        gitignore
            .lines()
            .filter(|line| line.trim() == ".gitsecret/keys/random_seed")
            .count(),
        1
    );
    assert_eq!(
        gitignore
            .lines()
            .filter(|line| line.trim() == ".gitsecret/keys/*.lock")
            .count(),
        1
    );
    assert_eq!(
        gitignore
            .lines()
            .filter(|line| line.trim() == "!*.secret")
            .count(),
        1
    );
}

#[test]
fn init_adds_gitattributes_diff_driver_without_duplicates() {
    let repo = TempRepo::new();
    run_success(Command::new("git").arg("init").arg(repo.path()));
    fs::write(
        repo.path().join(".gitattributes"),
        "*.txt text\n*.secret diff=git-secret\nexisting attr",
    )
    .expect("existing .gitattributes should be written");

    run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("init")
            .current_dir(repo.path()),
    );
    run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("init")
            .current_dir(repo.path()),
    );

    let gitattributes = fs::read_to_string(repo.path().join(".gitattributes"))
        .expect(".gitattributes should be readable");
    assert!(gitattributes.contains("*.txt text\n"));
    assert!(gitattributes.contains("existing attr"));
    assert_eq!(
        gitattributes
            .lines()
            .filter(|line| line.trim() == "*.secret diff=git-secret")
            .count(),
        1
    );

    let textconv = run_success(
        Command::new("git")
            .arg("config")
            .arg("--get")
            .arg("diff.git-secret.textconv")
            .current_dir(repo.path()),
    );
    assert_eq!(
        String::from_utf8_lossy(&textconv.stdout).trim(),
        "git-secret textconv"
    );
}

#[test]
fn init_uses_custom_secrets_extension_for_git_files() {
    let repo = TempRepo::new();
    run_success(Command::new("git").arg("init").arg(repo.path()));

    run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("init")
            .env("SECRETS_EXTENSION", ".enc")
            .current_dir(repo.path()),
    );

    let gitignore =
        fs::read_to_string(repo.path().join(".gitignore")).expect(".gitignore should be readable");
    assert!(gitignore.contains("!*.enc\n"));
    assert!(!gitignore.contains("!*.secret\n"));

    let gitattributes = fs::read_to_string(repo.path().join(".gitattributes"))
        .expect(".gitattributes should be readable");
    assert!(gitattributes.contains("*.enc diff=git-secret\n"));
    assert!(!gitattributes.contains("*.secret diff=git-secret\n"));
}

#[test]
fn init_upgrade_preserves_existing_keyring_and_mapping_while_refreshing_git_files() {
    let repo = TempRepo::new();
    run_success(Command::new("git").arg("init").arg(repo.path()));

    let gitsecret = repo.path().join(".gitsecret");
    let keys = gitsecret.join("keys");
    let paths = gitsecret.join("paths");
    let mapping = paths.join("mapping.cfg");
    let keybox = keys.join("pubring.kbx");
    fs::create_dir_all(&keys).expect("keys directory should be created");
    fs::create_dir_all(&paths).expect("paths directory should be created");
    fs::write(&keybox, "existing keyring").expect("existing keyring should be written");
    fs::write(&mapping, "existing mapping\n").expect("existing mapping should be written");
    fs::write(repo.path().join(".gitignore"), "target\n").expect(".gitignore should be written");
    fs::write(repo.path().join(".gitattributes"), "*.txt text\n")
        .expect(".gitattributes should be written");

    run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("init")
            .arg("--upgrade")
            .current_dir(repo.path()),
    );

    assert_eq!(
        fs::read_to_string(&keybox).expect("keyring should be readable"),
        "existing keyring"
    );
    assert_eq!(
        fs::read_to_string(&mapping).expect("mapping should be readable"),
        "existing mapping\n"
    );

    let gitignore =
        fs::read_to_string(repo.path().join(".gitignore")).expect(".gitignore should be readable");
    assert!(gitignore.contains("target\n"));
    assert!(gitignore.contains(".gitsecret/keys/random_seed\n"));
    assert!(gitignore.contains(".gitsecret/keys/*.lock\n"));
    assert!(gitignore.contains("!*.secret\n"));

    let gitattributes = fs::read_to_string(repo.path().join(".gitattributes"))
        .expect(".gitattributes should be readable");
    assert!(gitattributes.contains("*.txt text\n"));
    assert!(gitattributes.contains("*.secret diff=git-secret\n"));

    let textconv = run_success(
        Command::new("git")
            .arg("config")
            .arg("--get")
            .arg("diff.git-secret.textconv")
            .current_dir(repo.path()),
    );
    assert_eq!(
        String::from_utf8_lossy(&textconv.stdout).trim(),
        "git-secret textconv"
    );
}
