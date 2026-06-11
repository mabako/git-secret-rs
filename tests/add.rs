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
    assert_eq!(mapping, "config/secret.env:\n");
}

#[test]
fn add_and_remove_paths_are_relative_to_current_subdirectory() {
    let repo = TempRepo::new("gsadd-subdir");
    run_success(Command::new("git").arg("init").arg(repo.path()));
    run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("init")
            .current_dir(repo.path()),
    );

    let nested_dir = repo.path().join("foo");
    fs::create_dir_all(&nested_dir).expect("nested test dir should be created");
    fs::write(nested_dir.join("bar.txt"), "secret").expect("secret should be written");

    run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("add")
            .arg("bar.txt")
            .current_dir(&nested_dir),
    );

    let mapping = fs::read_to_string(repo.path().join(".gitsecret/paths/mapping.cfg"))
        .expect("mapping should be readable");
    assert_eq!(mapping, "foo/bar.txt:\n");

    run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("remove")
            .arg("bar.txt")
            .current_dir(&nested_dir),
    );

    let mapping = fs::read_to_string(repo.path().join(".gitsecret/paths/mapping.cfg"))
        .expect("mapping should be readable");
    assert_eq!(mapping, "");
}
