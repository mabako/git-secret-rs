use std::process::Command;

mod support;

use support::{run_success, TempRepo};

fn assert_git_secret_help(args: &[&str], expected: &[&str]) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_git-secret"));
    command.args(args);
    let output = run_success(&mut command);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Usage:"),
        "help output should contain Usage:\n{stdout}"
    );
    assert!(
        stdout.contains("--help"),
        "help output should contain --help:\n{stdout}"
    );
    for expected in expected {
        assert!(
            stdout.contains(expected),
            "help output should contain {expected}:\n{stdout}"
        );
    }
}

#[test]
fn init_help_prints_usage_without_creating_gitsecret() {
    let repo = TempRepo::new("git-secret-init-help");
    run_success(Command::new("git").arg("init").arg(repo.path()));

    let output = run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("init")
            .arg("-h")
            .current_dir(repo.path()),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"));
    assert!(stdout.contains("--help"));
    assert!(!repo.path().join(".gitsecret").exists());
}

#[test]
fn add_help_prints_usage() {
    assert_git_secret_help(&["add", "-h"], &[]);
}

#[test]
fn list_help_prints_usage() {
    assert_git_secret_help(&["list", "-h"], &[]);
}

#[test]
fn remove_help_prints_usage() {
    assert_git_secret_help(&["remove", "-h"], &["-c"]);
}

#[test]
fn clean_help_prints_usage() {
    assert_git_secret_help(&["clean", "-h"], &["-v"]);
}

#[test]
fn cat_help_prints_usage() {
    assert_git_secret_help(&["cat", "-h"], &["-d", "-p"]);
}

#[test]
fn changes_help_prints_usage() {
    assert_git_secret_help(&["changes", "-h"], &["-d", "-p"]);
}

#[test]
fn hide_help_prints_usage() {
    assert_git_secret_help(&["hide", "-h"], &["-c", "-F", "-P", "-d", "-m", "-v"]);
}

#[test]
fn reveal_help_prints_usage() {
    assert_git_secret_help(
        &["reveal", "-h"],
        &["-a", "--always-decrypt", "-f", "-F", "-d", "-v", "-p", "-P"],
    );
}

#[test]
fn tell_help_prints_usage() {
    assert_git_secret_help(&["tell", "-h"], &["-m", "-d"]);
}

#[test]
fn whoknows_help_prints_usage() {
    assert_git_secret_help(&["whoknows", "-h"], &["-l"]);
}

#[test]
fn removeperson_help_prints_usage() {
    assert_git_secret_help(&["removeperson", "-h"], &[]);
}
