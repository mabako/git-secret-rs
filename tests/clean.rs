use std::fs;
use std::process::Command;

mod support;

use support::{run_success, TempRepo};

#[test]
fn clean_deletes_secret_files_quietly_unless_verbose() {
    let repo = TempRepo::new("gsclean");
    run_success(Command::new("git").arg("init").arg(repo.path()));
    run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("init")
            .current_dir(repo.path()),
    );

    let nested_dir = repo.path().join("nested");
    fs::create_dir_all(&nested_dir).expect("nested test dir should be created");
    let root_secret = repo.path().join("root.txt.secret");
    let nested_secret = nested_dir.join("nested.txt.secret");
    let plaintext = repo.path().join("keep.txt");
    fs::write(&root_secret, "encrypted root").expect("root secret should be written");
    fs::write(&nested_secret, "encrypted nested").expect("nested secret should be written");
    fs::write(&plaintext, "plaintext").expect("plaintext should be written");

    let quiet = run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("clean")
            .current_dir(repo.path()),
    );
    assert_eq!(String::from_utf8_lossy(&quiet.stdout), "");
    assert!(!root_secret.exists());
    assert!(!nested_secret.exists());
    assert!(plaintext.exists());

    fs::write(&root_secret, "encrypted root").expect("root secret should be written again");
    fs::write(&nested_secret, "encrypted nested").expect("nested secret should be written again");
    let verbose = run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("clean")
            .arg("-v")
            .current_dir(repo.path()),
    );
    let verbose = String::from_utf8_lossy(&verbose.stdout);
    assert!(verbose.contains("removed root.txt.secret"));
    assert!(verbose.contains("removed nested/nested.txt.secret"));

    fs::write(&root_secret, "encrypted root").expect("root secret should be written again");
    let env_verbose = run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("clean")
            .env("SECRETS_VERBOSE", "1")
            .current_dir(repo.path()),
    );
    let env_verbose = String::from_utf8_lossy(&env_verbose.stdout);
    assert!(env_verbose.contains("removed root.txt.secret"));

    let help = run_success(
        Command::new(env!("CARGO_BIN_EXE_git-secret"))
            .arg("clean")
            .arg("-h"),
    );
    let help = String::from_utf8_lossy(&help.stdout);
    assert!(help.contains("Usage:"));
    assert!(help.contains("-v"));
    assert!(help.contains("--help"));
}
