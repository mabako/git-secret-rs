use std::process::Command;

mod support;

use support::{fixture_path, run_success, TempDir, TempRepo};

const USER1_FINGERPRINT: &str = "CE82DD3AFC167295F9132371D2805A4182E99FF4";
const USER1_UID: &str = "user1 <user1@gitsecret.io>";

#[test]
fn tell_and_removeperson_accept_fingerprint() {
    let user_gpg_home = TempDir::new("guser");
    run_success(
        Command::new("gpg")
            .arg("--homedir")
            .arg(user_gpg_home.path())
            .arg("--batch")
            .arg("--import")
            .arg(fixture_path("keys/public.key")),
    );

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
            .arg(USER1_FINGERPRINT)
            .env("GNUPGHOME", user_gpg_home.path())
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
        format!("{}\tnever", USER1_UID)
    );

    let whoknows_help = run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("whoknows")
            .arg("-h"),
    );
    let whoknows_help = String::from_utf8_lossy(&whoknows_help.stdout);
    assert!(whoknows_help.contains("git-secret-whoknows"));
    assert!(whoknows_help.contains("-l"));
    assert!(whoknows_help.contains("-h"));

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
