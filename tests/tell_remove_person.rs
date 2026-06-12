use std::process::Command;

mod support;

use support::{fixture_path, import_public_key, run_success, TempDir, TempRepo};

const USER1_FINGERPRINT: &str = "CE82DD3AFC167295F9132371D2805A4182E99FF4";
const USER1_UID: &str = "user1 <user1@gitsecret.io>";
const DUPLICATE_EMAIL: &str = "duplicate@example.com";

#[test]
fn tell_and_removeperson_accept_fingerprint() {
    let user_gpg_home = TempDir::new("guser");
    import_public_key(user_gpg_home.path(), &fixture_path("keys/public.key"));

    let repo = TempRepo::new("gstr");
    run_success(Command::new("git").arg("init").arg(repo.path()));
    run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("init")
            .current_dir(repo.path()),
    );

    run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("tell")
            .arg("-d")
            .arg(user_gpg_home.path())
            .arg(USER1_FINGERPRINT)
            .current_dir(repo.path()),
    );

    let whoknows = run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("whoknows")
            .current_dir(repo.path()),
    );
    assert!(
        String::from_utf8_lossy(&whoknows.stdout).contains(USER1_UID),
        "fingerprint tell should add the user:\n{}",
        String::from_utf8_lossy(&whoknows.stdout)
    );

    let whoknows_long = run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("whoknows")
            .arg("-l")
            .current_dir(repo.path()),
    );
    assert_eq!(
        String::from_utf8_lossy(&whoknows_long.stdout).trim(),
        format!("{} (expires: never)", USER1_UID)
    );

    run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("removeperson")
            .arg(USER1_FINGERPRINT)
            .current_dir(repo.path()),
    );

    let whoknows = run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("whoknows")
            .current_dir(repo.path()),
    );
    assert_eq!(
        String::from_utf8_lossy(&whoknows.stdout).trim(),
        "no recipients configured"
    );
}

#[test]
fn tell_can_use_git_email() {
    let user_gpg_home = TempDir::new("guser");
    import_public_key(user_gpg_home.path(), &fixture_path("keys/public.key"));

    let repo = TempRepo::new("gstm");
    run_success(Command::new("git").arg("init").arg(repo.path()));
    run_success(
        Command::new("git")
            .arg("config")
            .arg("user.email")
            .arg("user1@gitsecret.io")
            .current_dir(repo.path()),
    );
    run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("init")
            .current_dir(repo.path()),
    );

    run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("tell")
            .arg("-d")
            .arg(user_gpg_home.path())
            .arg("-m")
            .current_dir(repo.path()),
    );

    let whoknows = run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("whoknows")
            .current_dir(repo.path()),
    );
    assert!(
        String::from_utf8_lossy(&whoknows.stdout).contains(USER1_UID),
        "tell -m should add the git user.email key:\n{}",
        String::from_utf8_lossy(&whoknows.stdout)
    );
}

#[test]
fn tell_rejects_email_that_matches_multiple_local_keys() {
    let user_gpg_home = TempDir::new("guser-duplicate");
    import_public_key(
        user_gpg_home.path(),
        &fixture_path("keys/duplicate-public-keys.asc"),
    );

    let repo = TempRepo::new("gstd");
    run_success(Command::new("git").arg("init").arg(repo.path()));
    run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("init")
            .current_dir(repo.path()),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_git-secret"))
        .arg("tell")
        .arg("-d")
        .arg(user_gpg_home.path())
        .arg(DUPLICATE_EMAIL)
        .current_dir(repo.path())
        .output()
        .expect("git-secret tell should run");
    assert!(
        !output.status.success(),
        "tell should fail when an email matches multiple keys\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("multiple public keys match 'duplicate@example.com'"),
        "tell should report ambiguous matching keys:\n{}",
        stderr
    );
}
