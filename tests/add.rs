use std::fs;
use std::process::Command;

mod support;

use support::{run_success, TempRepo};

#[test]
fn add_tracks_secret_and_ignores_plaintext_file() {
    let repo = TempRepo::new("gsadd");
    run_success(Command::new("git").arg("init").arg(repo.path()));
    run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("init")
            .current_dir(repo.path()),
    );

    let nested_dir = repo.path().join("config");
    fs::create_dir_all(&nested_dir).expect("nested test dir should be created");
    fs::write(nested_dir.join("secret.env"), "API_TOKEN=abc123").expect("secret should be written");

    run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("add")
            .arg("config/secret.env")
            .current_dir(repo.path()),
    );
    run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("add")
            .arg("config/secret.env")
            .current_dir(repo.path()),
    );

    let gitignore =
        fs::read_to_string(repo.path().join(".gitignore")).expect(".gitignore should be created");
    assert_eq!(
        gitignore
            .lines()
            .filter(|line| line.trim() == "config/secret.env")
            .count(),
        1,
        ".gitignore should contain the plaintext path once:\n{}",
        gitignore
    );

    let mapping = fs::read_to_string(repo.path().join(".gitsecret/paths/mapping.cfg"))
        .expect("mapping should be readable");
    assert!(
        mapping.starts_with("config/secret.env:"),
        "mapping should contain the tracked secret:\n{}",
        mapping
    );
}
