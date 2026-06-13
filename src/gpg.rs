use std::env;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;

const PROGRAM_FILES_X86_ENV: &str = "ProgramFiles(x86)";
const SECRETS_GPG_COMMAND_ENV: &str = "SECRETS_GPG_COMMAND";
const SECRETS_PINENTRY_ENV: &str = "SECRETS_PINENTRY";

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

#[cfg(test)]
mod tests {
    use super::*;

    fn command_args(command: &Command) -> Vec<String> {
        command
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect()
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
}
