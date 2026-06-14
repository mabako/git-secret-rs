#![cfg(windows)]

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

mod support;

use support::{
    assert_failure, fixture_key_passphrase, fixture_key_path, run_success, TempDir, TempRepo,
};

const USER1_EMAIL: &str = "user1@gitsecret.io";
const USER1_FINGERPRINT: &str = "CE82DD3AFC167295F9132371D2805A4182E99FF4";
const USER1_UID: &str = "user1 <user1@gitsecret.io>";

#[test]
fn all_commands_work_with_git_bash_gpg() {
    let Some(gpg) = git_bash_gpg() else {
        eprintln!("skipping Git Bash gpg compatibility test; usr/bin/gpg.exe was not found");
        return;
    };

    let repo = TempRepo::new();
    run_success(Command::new("git").arg("init").arg(repo.path()));

    run_git_secret(&gpg, ["help"], repo.path());
    run_git_secret(&gpg, ["usage"], repo.path());
    run_git_secret(&gpg, ["version"], repo.path());
    run_git_secret(&gpg, ["init"], repo.path());

    let user_gpg_home = TempDir::new();
    let user_passphrase = fixture_key_passphrase(USER1_EMAIL);
    import_public_key(
        &gpg,
        user_gpg_home.path(),
        fixture_key_path(USER1_EMAIL, "public.key"),
    );
    import_private_key(
        &gpg,
        user_gpg_home.path(),
        fixture_key_path(USER1_EMAIL, "private.key"),
        &user_passphrase,
    );

    run_git_secret(
        &gpg,
        [
            OsStr::new("tell"),
            OsStr::new("-d"),
            user_gpg_home.path().as_os_str(),
            OsStr::new(USER1_FINGERPRINT),
        ],
        repo.path(),
    );

    let whoknows = run_git_secret(&gpg, ["whoknows"], repo.path());
    assert!(
        String::from_utf8_lossy(&whoknows.stdout).contains(USER1_UID),
        "whoknows should list the recipient imported through Git Bash gpg:\n{}",
        String::from_utf8_lossy(&whoknows.stdout)
    );

    let whoknows_long = run_git_secret(&gpg, ["whoknows", "-l"], repo.path());
    assert!(
        String::from_utf8_lossy(&whoknows_long.stdout).contains("(expires: never)"),
        "whoknows -l should include expiration details:\n{}",
        String::from_utf8_lossy(&whoknows_long.stdout)
    );

    let plaintext = repo.path().join("secret.txt");
    fs::write(&plaintext, "the launch code is swordfish")
        .expect("plaintext secret should be written");
    run_git_secret(&gpg, ["add", "secret.txt"], repo.path());

    let list = run_git_secret(&gpg, ["list"], repo.path());
    assert_eq!(String::from_utf8_lossy(&list.stdout).trim(), "secret.txt");

    run_git_secret(&gpg, ["hide"], repo.path());
    let encrypted = repo.path().join("secret.txt.secret");
    assert!(encrypted.is_file(), "{} should exist", encrypted.display());

    let changes = run_git_secret(
        &gpg,
        [
            OsStr::new("changes"),
            OsStr::new("-d"),
            user_gpg_home.path().as_os_str(),
            OsStr::new("-p"),
            OsStr::new(&user_passphrase),
        ],
        repo.path(),
    );
    assert!(
        String::from_utf8_lossy(&changes.stdout).contains("changes in"),
        "changes should list checked files:\n{}",
        String::from_utf8_lossy(&changes.stdout)
    );

    let cat = run_git_secret(
        &gpg,
        [
            OsStr::new("cat"),
            OsStr::new("-d"),
            user_gpg_home.path().as_os_str(),
            OsStr::new("-p"),
            OsStr::new(&user_passphrase),
            OsStr::new("secret.txt"),
        ],
        repo.path(),
    );
    assert_eq!(
        String::from_utf8_lossy(&cat.stdout),
        "the launch code is swordfish"
    );

    let textconv = run_git_secret(
        &gpg,
        [
            OsStr::new("textconv"),
            OsStr::new("-d"),
            user_gpg_home.path().as_os_str(),
            OsStr::new("-p"),
            OsStr::new(&user_passphrase),
            encrypted.as_os_str(),
        ],
        repo.path(),
    );
    assert_eq!(
        String::from_utf8_lossy(&textconv.stdout),
        "the launch code is swordfish"
    );

    fs::remove_file(&plaintext).expect("plaintext should be removed before reveal");
    run_git_secret(
        &gpg,
        [
            OsStr::new("reveal"),
            OsStr::new("-d"),
            user_gpg_home.path().as_os_str(),
            OsStr::new("-p"),
            OsStr::new(&user_passphrase),
        ],
        repo.path(),
    );
    assert_eq!(
        fs::read_to_string(&plaintext).expect("revealed plaintext should be readable"),
        "the launch code is swordfish"
    );

    fs::write(repo.path().join("leftover.secret"), "stale encrypted file")
        .expect("leftover secret should be written");
    run_git_secret(&gpg, ["clean"], repo.path());
    assert!(
        !repo.path().join("leftover.secret").exists(),
        "clean should delete .secret files"
    );

    run_git_secret(&gpg, ["remove", "-c", "secret.txt"], repo.path());
    let list = git_secret_output(&gpg, ["list"], repo.path());
    assert_failure(&list);

    run_git_secret(&gpg, ["removeperson", USER1_FINGERPRINT], repo.path());
    let whoknows = git_secret_output(&gpg, ["whoknows"], repo.path());
    assert_failure(&whoknows);
    assert!(
        String::from_utf8_lossy(&whoknows.stderr).contains("no recipients configured"),
        "whoknows should report no recipients:\n{}",
        String::from_utf8_lossy(&whoknows.stderr)
    );
}

fn run_git_secret<I, S>(gpg: &Path, args: I, current_dir: &Path) -> std::process::Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = Command::new(env!("CARGO_BIN_EXE_git-secret"));
    command
        .args(args)
        .env("SECRETS_GPG_COMMAND", gpg)
        .current_dir(current_dir);
    run_success(&mut command)
}

fn git_secret_output<I, S>(gpg: &Path, args: I, current_dir: &Path) -> std::process::Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = Command::new(env!("CARGO_BIN_EXE_git-secret"));
    command
        .args(args)
        .env("SECRETS_GPG_COMMAND", gpg)
        .current_dir(current_dir);
    command.output().expect("git-secret command should run")
}

fn import_public_key(gpg: &Path, homedir: &Path, key: PathBuf) {
    run_success(
        Command::new(gpg)
            .arg("--homedir")
            .arg(git_bash_arg_path(homedir))
            .arg("--batch")
            .arg("--import")
            .arg(git_bash_arg_path(&key)),
    );
}

fn import_private_key(gpg: &Path, homedir: &Path, key: PathBuf, passphrase: &str) {
    run_success(
        Command::new(gpg)
            .arg("--homedir")
            .arg(git_bash_arg_path(homedir))
            .arg("--batch")
            .arg("--pinentry-mode")
            .arg("loopback")
            .arg("--passphrase")
            .arg(passphrase)
            .arg("--import")
            .arg(git_bash_arg_path(&key)),
    );
}

fn git_bash_gpg() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("GIT_BASH_GPG_COMMAND").map(PathBuf::from) {
        if path.is_file() {
            return Some(path);
        }
    }

    git_bash_gpg_candidates()
        .into_iter()
        .find(|path| path.is_file())
}

fn git_bash_gpg_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for env_name in ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"] {
        if let Some(root) = std::env::var_os(env_name) {
            candidates.push(
                PathBuf::from(root)
                    .join("Git")
                    .join("usr")
                    .join("bin")
                    .join("gpg.exe"),
            );
        }
    }
    candidates
}

#[cfg(windows)]
fn git_bash_arg_path(path: &Path) -> std::ffi::OsString {
    let path = path.to_string_lossy().replace('\\', "/");
    let bytes = path.as_bytes();
    if bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'/' {
        let drive = (bytes[0] as char).to_ascii_lowercase();
        return format!("/{drive}{}", &path[2..]).into();
    }

    path.into()
}

#[cfg(not(windows))]
fn git_bash_arg_path(path: &Path) -> std::ffi::OsString {
    path.as_os_str().to_os_string()
}
