#![allow(dead_code)]

use std::ffi::OsString;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) struct TempRepo {
    path: PathBuf,
}

pub(crate) struct TempDir {
    path: PathBuf,
}

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

impl TempRepo {
    pub(crate) fn new(prefix: &str) -> Self {
        Self {
            path: create_temp_path(prefix),
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

impl TempDir {
    pub(crate) fn new(prefix: &str) -> Self {
        Self {
            path: create_temp_path(prefix),
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn create_temp_path(prefix: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();

    for _ in 0..100 {
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "{}-{:x}-{:x}-{:x}",
            prefix,
            std::process::id(),
            unique,
            counter
        ));

        match fs::create_dir(&path) {
            Ok(()) => {
                set_private_directory_permissions(&path);
                return path;
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => panic!("temp directory should be created: {error}"),
        }
    }

    panic!("unique temp directory should be created");
}

#[cfg(unix)]
fn set_private_directory_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .expect("temp directory permissions should be restricted");
}

#[cfg(not(unix))]
fn set_private_directory_permissions(_path: &Path) {}

pub(crate) fn run_success(command: &mut Command) -> Output {
    let output = command.output().expect("command should run");
    assert!(
        output.status.success(),
        "command failed with {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

pub(crate) fn git_secret(current_dir: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_git-secret"));
    command.current_dir(current_dir);
    command
}

pub(crate) fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed with {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

pub(crate) fn assert_failure(output: &Output) {
    assert!(
        !output.status.success(),
        "command unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

pub(crate) fn gpg_command() -> Command {
    Command::new(gpg_program())
}

fn gpg_program() -> PathBuf {
    if let Some(secrets_gpg_command) = std::env::var_os("SECRETS_GPG_COMMAND") {
        return PathBuf::from(secrets_gpg_command);
    }

    if std::env::var("MSYSTEM").ok().as_deref() == Some("MINGW64") {
        if let Some(program_files_x86) = std::env::var_os("ProgramFiles(x86)") {
            return PathBuf::from(program_files_x86)
                .join("GnuPG")
                .join("bin")
                .join("gpg.exe");
        }
    }

    PathBuf::from("gpg")
}

pub(crate) fn gpg_arg_path(path: &Path) -> OsString {
    if gpg_uses_msys_paths() {
        return msys_path(path).unwrap_or_else(|| path.as_os_str().to_os_string());
    }

    path.as_os_str().to_os_string()
}

fn gpg_uses_msys_paths() -> bool {
    let program = gpg_program();
    if gpg_program_needs_msys_paths(&program) {
        return true;
    }

    program == PathBuf::from("gpg") && std::env::var_os("MSYSTEM").is_some()
}

fn gpg_program_needs_msys_paths(program: &Path) -> bool {
    let Some(file_name) = program.file_name() else {
        return false;
    };
    let file_name = file_name.to_string_lossy();
    if !file_name.eq_ignore_ascii_case("gpg") && !file_name.eq_ignore_ascii_case("gpg.exe") {
        return false;
    }

    let mut components = program
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    if components.len() < 3 {
        return false;
    }

    components.reverse();
    components[1].eq_ignore_ascii_case("bin") && components[2].eq_ignore_ascii_case("usr")
}

#[cfg(windows)]
fn msys_path(path: &Path) -> Option<OsString> {
    let path = path.to_string_lossy().replace('\\', "/");
    let bytes = path.as_bytes();
    if bytes.len() < 3 || !bytes[0].is_ascii_alphabetic() || bytes[1] != b':' || bytes[2] != b'/' {
        return None;
    }

    let drive = (bytes[0] as char).to_ascii_lowercase();
    Some(OsString::from(format!("/{drive}{}", &path[2..])))
}

#[cfg(not(windows))]
fn msys_path(_path: &Path) -> Option<OsString> {
    None
}

pub(crate) fn import_public_key(homedir: &Path, key: &Path) {
    let output = gpg_command()
        .arg("--homedir")
        .arg(gpg_arg_path(homedir))
        .arg("--batch")
        .arg("--status-fd")
        .arg("1")
        .arg("--import")
        .arg(gpg_arg_path(key))
        .output()
        .expect("gpg import command should run");
    assert!(
        output.status.success() || gpg_import_succeeded(&output.stdout),
        "gpg public key import failed with {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

pub(crate) fn import_public_fixture_key(homedir: &Path, email: &str) {
    import_public_key(homedir, &fixture_key_path(email, "public.key"));
}

pub(crate) fn import_private_fixture_key(homedir: &Path, email: &str) {
    import_private_key(
        homedir,
        &fixture_key_path(email, "private.key"),
        &fixture_key_passphrase(email),
    );
}

pub(crate) fn import_private_key(homedir: &Path, key: &Path, passphrase: &str) {
    run_success(
        gpg_command()
            .arg("--homedir")
            .arg(gpg_arg_path(homedir))
            .arg("--batch")
            .arg("--pinentry-mode")
            .arg("loopback")
            .arg("--passphrase")
            .arg(passphrase)
            .arg("--import")
            .arg(gpg_arg_path(key)),
    );
}

pub(crate) fn fixture_key_path(email: &str, file: &str) -> PathBuf {
    fixture_path(&format!("keys/{email}/{file}"))
}

pub(crate) fn fixture_key_passphrase(email: &str) -> String {
    let local_part = email
        .split_once('@')
        .map(|(local_part, _)| local_part)
        .unwrap_or(email);
    format!("{local_part}pass")
}

fn gpg_import_succeeded(stdout: &[u8]) -> bool {
    String::from_utf8_lossy(stdout)
        .lines()
        .any(|line| line.starts_with("[GNUPG:] IMPORT_OK "))
}

pub(crate) fn fixture_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(path)
}
