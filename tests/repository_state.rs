use std::fs;
use std::process::Command;

mod support;

use support::{assert_failure, assert_success, git_secret, run_success, TempDir, TempRepo};

#[test]
fn non_help_commands_fail_outside_git_repository() {
    let dir = TempDir::new();

    for args in repository_commands() {
        let output = git_secret(dir.path())
            .args(&args)
            .output()
            .expect("git-secret command should run");

        assert_failure(&output);
        assert_stderr_contains(&output, "not inside a git repository");
    }
}

#[test]
fn help_usage_and_version_do_not_validate_repository_state() {
    let dir = TempDir::new();

    for args in [
        Vec::<&str>::new(),
        vec!["help"],
        vec!["usage"],
        vec!["version"],
    ] {
        let output = git_secret(dir.path())
            .args(&args)
            .output()
            .expect("git-secret command should run");

        assert_success(&output);
    }
}

#[test]
fn non_help_commands_fail_when_secret_dir_is_ignored() {
    let repo = initialized_repo();
    fs::write(repo.path().join(".gitignore"), ".gitsecret/\n")
        .expect(".gitignore should be written");

    for args in repository_commands() {
        let output = git_secret(repo.path())
            .args(&args)
            .output()
            .expect("git-secret command should run");

        assert_failure(&output);
        assert_stderr_contains(&output, "secret directory .gitsecret must not be ignored");
    }
}

#[test]
fn init_fails_when_custom_secret_dir_is_ignored() {
    let repo = TempRepo::new();
    run_success(Command::new("git").arg("init").arg(repo.path()));
    fs::write(repo.path().join(".gitignore"), ".secrets/\n").expect(".gitignore should be written");

    let output = git_secret(repo.path())
        .arg("init")
        .env("SECRETS_DIR", ".secrets")
        .output()
        .expect("git-secret init should run");

    assert_failure(&output);
    assert_stderr_contains(&output, "secret directory .secrets must not be ignored");
}

#[test]
fn non_help_commands_fail_when_repository_keyring_contains_secret_keys() {
    let repo = initialized_repo();
    let secret_keyring = repo
        .path()
        .join(".gitsecret")
        .join("keys")
        .join("secring.gpg");
    fs::write(&secret_keyring, "private key").expect("secret keyring should be written");

    for args in repository_commands() {
        let output = git_secret(repo.path())
            .args(&args)
            .output()
            .expect("git-secret command should run");

        assert_failure(&output);
        assert_stderr_contains(&output, "repository keyring contains secret keys");
    }
}

fn initialized_repo() -> TempRepo {
    let repo = TempRepo::new();
    run_success(Command::new("git").arg("init").arg(repo.path()));
    run_success(git_secret(repo.path()).arg("init"));
    repo
}

fn repository_commands() -> Vec<Vec<&'static str>> {
    vec![
        vec!["add", "secret.txt"],
        vec!["cat", "secret.txt"],
        vec!["changes"],
        vec!["clean"],
        vec!["hide"],
        vec!["init"],
        vec!["list"],
        vec!["remove", "secret.txt"],
        vec!["removeperson", "user@example.com"],
        vec!["reveal"],
        vec!["tell", "user@example.com"],
        vec!["textconv", "secret.txt.secret"],
        vec!["whoknows"],
    ]
}

fn assert_stderr_contains(output: &std::process::Output, expected: &str) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(expected),
        "stderr should contain {expected:?}:\n{stderr}"
    );
}
