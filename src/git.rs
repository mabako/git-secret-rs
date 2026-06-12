use std::env;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::AppResult;

const DEFAULT_SECRET_DIR: &str = ".gitsecret";
const KEYS_DIR_NAME: &str = "keys";
const PATHS_DIR_NAME: &str = "paths";
const MAPPING_FILE_NAME: &str = "mapping.cfg";
const PROGRAM_FILES_X86_ENV: &str = "ProgramFiles(x86)";
const SECRETS_GPG_COMMAND_ENV: &str = "SECRETS_GPG_COMMAND";
const SECRETS_DIR_ENV: &str = "SECRETS_DIR";
const SECRETS_PINENTRY_ENV: &str = "SECRETS_PINENTRY";

pub(crate) struct RecipientRecord {
    pub(crate) uid: String,
    pub(crate) expires: String,
}

pub(crate) struct Repo {
    root: PathBuf,
}

impl Repo {
    pub(crate) fn discover() -> AppResult<Self> {
        let output = Command::new("git")
            .arg("rev-parse")
            .arg("--show-toplevel")
            .output()
            .map_err(|e| format!("run git rev-parse: {}", e))?;

        if !output.status.success() {
            return Err("not inside a git repository".to_string());
        }

        let root = String::from_utf8(output.stdout)
            .map_err(|_| "git returned a non-UTF-8 repository path".to_string())?;
        Ok(Self {
            root: PathBuf::from(root.trim()),
        })
    }

    pub(crate) fn join<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        self.root.join(path)
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }
}

pub(crate) fn ensure_initialized(repo: &Repo) -> AppResult<()> {
    if !repo.join(mapping_file()).is_file() {
        return Err("repository is not initialized; run 'git secret init' first".to_string());
    }

    Ok(())
}

pub(crate) fn repo_gpg(repo: &Repo) -> Command {
    let mut command = gpg_command();
    command
        .arg("--homedir")
        .arg(gpg_arg_path(&repo.join(keys_dir())));
    command
}

pub(crate) fn secret_dir() -> PathBuf {
    env::var_os(SECRETS_DIR_ENV)
        .map(PathBuf::from)
        .filter(|path| is_valid_secret_dir(path))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_SECRET_DIR))
}

pub(crate) fn keys_dir() -> PathBuf {
    secret_dir().join(KEYS_DIR_NAME)
}

pub(crate) fn paths_dir() -> PathBuf {
    secret_dir().join(PATHS_DIR_NAME)
}

pub(crate) fn mapping_file() -> PathBuf {
    paths_dir().join(MAPPING_FILE_NAME)
}

fn is_valid_secret_dir(path: &Path) -> bool {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return false;
    }

    let mut has_component = false;
    for component in path.components() {
        use std::path::Component::*;
        match component {
            Normal(_) => has_component = true,
            CurDir | ParentDir | RootDir | Prefix(_) => return false,
        }
    }
    has_component
}

#[derive(Default, clap::Args)]
pub(crate) struct UserGpgOptions {
    #[arg(
        short = 'd',
        value_name = "gpg-homedir",
        help = "Specifies `--homedir` option for the `gpg`, use this option if you store your keys in a custom location"
    )]
    pub(crate) homedir: Option<PathBuf>,
    #[arg(
        short = 'p',
        value_name = "password",
        help = "Specifies password for noinput mode, adds `--passphrase` option for `gpg`"
    )]
    pub(crate) passphrase: Option<String>,
}

pub(crate) fn user_gpg(options: &UserGpgOptions) -> Command {
    user_gpg_with_pinentry(options, secrets_pinentry().as_deref())
}

fn user_gpg_with_pinentry(options: &UserGpgOptions, pinentry: Option<&OsStr>) -> Command {
    let mut command = gpg_command_with_pinentry(pinentry);
    command.arg("--quiet").arg("--no-tty");
    if let Some(homedir) = &options.homedir {
        command.arg("--homedir").arg(gpg_arg_path(homedir));
    }
    if let Some(passphrase) = &options.passphrase {
        if pinentry.is_none() {
            add_pinentry_mode(&mut command, OsStr::new("loopback"));
        }
        command.arg("--passphrase").arg(passphrase);
    }
    command
}

pub(crate) fn gpg_command() -> Command {
    gpg_command_with_pinentry(secrets_pinentry().as_deref())
}

fn gpg_command_with_pinentry(pinentry: Option<&OsStr>) -> Command {
    let program = gpg_program_from_env();
    let mut command = Command::new(&program);
    configure_gpg_environment(&mut command, &program);
    if let Some(pinentry) = pinentry {
        add_pinentry_mode(&mut command, pinentry);
    }
    command
}

pub(crate) fn gpg_arg_path(path: &Path) -> OsString {
    if gpg_needs_msys_paths() {
        return msys_path(path).unwrap_or_else(|| path.as_os_str().to_os_string());
    }

    path.as_os_str().to_os_string()
}

pub(crate) fn gpg_needs_msys_paths() -> bool {
    let program = gpg_program_from_env();
    gpg_program_needs_msys_paths(&program)
        || (program == PathBuf::from("gpg") && env::var_os("MSYSTEM").is_some())
}

fn gpg_program_from_env() -> PathBuf {
    let program_files_x86 = env::var_os(PROGRAM_FILES_X86_ENV).map(PathBuf::from);
    let secrets_gpg_command = env::var_os(SECRETS_GPG_COMMAND_ENV).map(PathBuf::from);
    gpg_program_for_env(
        secrets_gpg_command.as_deref(),
        env::var("MSYSTEM").ok().as_deref(),
        program_files_x86.as_deref(),
    )
}

fn configure_gpg_environment(command: &mut Command, program: &Path) {
    if !gpg_program_needs_msys_paths(program) {
        return;
    }

    let Some(bin_dir) = program.parent() else {
        return;
    };
    let mut paths = vec![bin_dir.to_path_buf()];
    if let Some(existing) = env::var_os("PATH") {
        paths.extend(env::split_paths(&existing));
    }
    if let Ok(path) = env::join_paths(paths) {
        command.env("PATH", path);
    }
}

fn secrets_pinentry() -> Option<OsString> {
    env::var_os(SECRETS_PINENTRY_ENV).filter(|value| !value.is_empty())
}

fn add_pinentry_mode(command: &mut Command, pinentry: &OsStr) {
    command.arg("--pinentry-mode").arg(pinentry);
}

fn gpg_program_for_env(
    secrets_gpg_command: Option<&Path>,
    msystem: Option<&str>,
    program_files_x86: Option<&Path>,
) -> PathBuf {
    if let Some(secrets_gpg_command) = secrets_gpg_command {
        return secrets_gpg_command.to_path_buf();
    }

    if msystem == Some("MINGW64") {
        if let Some(program_files_x86) = program_files_x86 {
            return program_files_x86.join("GnuPG").join("bin").join("gpg.exe");
        }
    }

    PathBuf::from("gpg")
}

fn gpg_program_needs_msys_paths(program: &Path) -> bool {
    let Some(file_name) = program.file_name() else {
        return false;
    };
    if !file_name.eq_ignore_ascii_case("gpg.exe") {
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
    components[1].eq_ignore_ascii_case("bin")
        && components[2].eq_ignore_ascii_case("usr")
        && components
            .iter()
            .skip(3)
            .any(|component| component.eq_ignore_ascii_case("Git"))
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

pub(crate) fn recipient_key_ids(repo: &Repo) -> AppResult<Vec<String>> {
    let output = repo_gpg(repo)
        .arg("--with-colons")
        .arg("--list-keys")
        .stdout(Stdio::piped())
        .output()
        .map_err(|e| format!("list recipients: {}", e))?;
    if !output.status.success() {
        return Err("could not list repository recipients".to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut recipients = Vec::new();
    for line in stdout.lines() {
        let fields: Vec<&str> = line.split(':').collect();
        if fields.first() == Some(&"pub") {
            if let Some(key_id) = fields.get(4) {
                if !key_id.is_empty() {
                    recipients.push((*key_id).to_string());
                }
            }
        }
    }

    Ok(recipients)
}

pub(crate) fn recipient_records(repo: &Repo) -> AppResult<Vec<RecipientRecord>> {
    let output = repo_gpg(repo)
        .arg("--with-colons")
        .arg("--list-keys")
        .stdout(Stdio::piped())
        .output()
        .map_err(|e| format!("list recipients: {}", e))?;
    if !output.status.success() {
        return Err("could not list repository recipients".to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut recipients = Vec::new();
    let mut current_expiration = "never".to_string();
    for line in stdout.lines() {
        let fields: Vec<&str> = line.split(':').collect();
        match fields.first().copied() {
            Some("pub") => {
                current_expiration = fields
                    .get(6)
                    .and_then(|expires| format_gpg_expiration(expires))
                    .unwrap_or_else(|| "never".to_string());
            }
            Some("uid") => {
                if let Some(uid) = fields.get(9) {
                    if !uid.is_empty() {
                        recipients.push(RecipientRecord {
                            uid: (*uid).to_string(),
                            expires: current_expiration.clone(),
                        });
                    }
                }
            }
            _ => {}
        }
    }

    Ok(recipients)
}

fn format_gpg_expiration(value: &str) -> Option<String> {
    if value.is_empty() {
        return None;
    }

    value
        .parse::<i64>()
        .ok()
        .map(|timestamp| format_unix_date(timestamp))
}

fn format_unix_date(timestamp: i64) -> String {
    let days = timestamp.div_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!("{:04}-{:02}-{:02}", year, month, day)
}

fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = year + if month <= 2 { 1 } else { 0 };

    (year as i32, month as u32, day as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    fn command_args(command: &Command) -> Vec<String> {
        command
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect()
    }

    #[test]
    fn format_gpg_expiration_handles_empty_values() {
        assert_eq!(format_gpg_expiration(""), None);
    }

    #[test]
    fn format_gpg_expiration_formats_unix_dates() {
        assert_eq!(
            format_gpg_expiration("1453490413"),
            Some("2016-01-22".to_string())
        );
    }

    #[test]
    fn gpg_program_uses_original_gnupg_under_mingw64() {
        let program_files_x86 = Path::new(r"C:\ProgramFilesX86");

        assert_eq!(
            gpg_program_for_env(None, Some("MINGW64"), Some(program_files_x86)),
            program_files_x86.join("GnuPG").join("bin").join("gpg.exe")
        );
    }

    #[test]
    fn gpg_program_uses_path_lookup_without_program_files_x86() {
        assert_eq!(
            gpg_program_for_env(None, Some("MINGW64"), None),
            PathBuf::from("gpg")
        );
    }

    #[test]
    fn gpg_program_uses_path_lookup_outside_mingw64() {
        assert_eq!(
            gpg_program_for_env(None, None, Some(Path::new(r"C:\ProgramFilesX86"))),
            PathBuf::from("gpg")
        );
        assert_eq!(
            gpg_program_for_env(None, Some("MSYS"), Some(Path::new(r"C:\ProgramFilesX86"))),
            PathBuf::from("gpg")
        );
    }

    #[test]
    fn gpg_program_uses_secrets_gpg_command_when_set() {
        assert_eq!(
            gpg_program_for_env(
                Some(Path::new("/usr/local/gpg")),
                Some("MINGW64"),
                Some(Path::new(r"C:\ProgramFilesX86"))
            ),
            PathBuf::from("/usr/local/gpg")
        );
        assert_eq!(
            gpg_program_for_env(Some(Path::new("gpg2")), None, None),
            PathBuf::from("gpg2")
        );
    }

    #[test]
    #[cfg(windows)]
    fn git_bash_gpg_program_uses_msys_paths() {
        assert!(gpg_program_needs_msys_paths(Path::new(
            r"C:\Program Files\Git\usr\bin\gpg.exe"
        )));
        assert!(!gpg_program_needs_msys_paths(Path::new(
            r"C:\Program Files (x86)\GnuPG\bin\gpg.exe"
        )));
    }

    #[test]
    #[cfg(windows)]
    fn msys_path_converts_windows_drive_paths() {
        assert_eq!(
            msys_path(Path::new(r"C:\Users\alice\AppData\Local\Temp")).unwrap(),
            OsString::from("/c/Users/alice/AppData/Local/Temp")
        );
        assert_eq!(msys_path(Path::new("relative/path")), None);
    }

    #[test]
    fn gpg_command_adds_configured_pinentry_mode() {
        let command = gpg_command_with_pinentry(Some(OsStr::new("ask")));

        assert_eq!(command_args(&command), vec!["--pinentry-mode", "ask"]);
    }

    #[test]
    fn user_gpg_uses_loopback_pinentry_for_passphrase_by_default() {
        let options = UserGpgOptions {
            homedir: None,
            passphrase: Some("secret".to_string()),
        };
        let command = user_gpg_with_pinentry(&options, None);

        assert_eq!(
            command_args(&command),
            vec![
                "--quiet",
                "--no-tty",
                "--pinentry-mode",
                "loopback",
                "--passphrase",
                "secret"
            ]
        );
    }

    #[test]
    fn user_gpg_prefers_configured_pinentry_mode() {
        let options = UserGpgOptions {
            homedir: None,
            passphrase: Some("secret".to_string()),
        };
        let command = user_gpg_with_pinentry(&options, Some(OsStr::new("cancel")));

        assert_eq!(
            command_args(&command),
            vec![
                "--pinentry-mode",
                "cancel",
                "--quiet",
                "--no-tty",
                "--passphrase",
                "secret"
            ]
        );
    }

    #[test]
    fn secret_dir_uses_valid_relative_paths() {
        assert!(is_valid_secret_dir(Path::new(".custom-secret")));
        assert!(is_valid_secret_dir(Path::new("secrets/store")));
        assert!(!is_valid_secret_dir(Path::new("")));
        assert!(!is_valid_secret_dir(Path::new(".")));
        assert!(!is_valid_secret_dir(Path::new("../secrets")));

        let absolute = std::env::current_dir().unwrap().join("secrets");
        assert!(!is_valid_secret_dir(&absolute));

        #[cfg(windows)]
        assert!(!is_valid_secret_dir(Path::new(r"C:\secrets")));
    }
}
