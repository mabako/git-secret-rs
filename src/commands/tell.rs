use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::git::{ensure_initialized, gpg_command, repo_gpg, Repo};
use crate::AppResult;

pub(crate) struct Options {
    use_git_email: bool,
    gpg_homedir: Option<PathBuf>,
    keys: Vec<String>,
    help: bool,
}

impl Options {
    pub(crate) fn parse(args: Vec<String>) -> AppResult<Self> {
        let mut use_git_email = false;
        let mut gpg_homedir = None;
        let mut keys = Vec::new();
        let mut help = false;
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-m" => use_git_email = true,
                "-d" => {
                    let homedir = args
                        .next()
                        .ok_or_else(|| "tell option -d requires a gpg homedir".to_string())?;
                    gpg_homedir = Some(PathBuf::from(homedir));
                }
                "-h" | "--help" => help = true,
                _ if arg.starts_with('-') => return Err(format!("unknown tell option '{}'", arg)),
                _ => keys.push(arg),
            }
        }

        Ok(Self {
            use_git_email,
            gpg_homedir,
            keys,
            help,
        })
    }
}

pub(crate) fn run(options: Options) -> AppResult<Vec<String>> {
    if options.help {
        print_help();
        return Ok(Vec::new());
    }

    let mut keys = options.keys;
    if options.use_git_email {
        keys.push(git_user_email()?);
    }

    if keys.is_empty() {
        return Err("tell requires at least one fingerprint, key id, or email".to_string());
    }

    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;
    let mut imported = Vec::new();

    for key in keys {
        let exported = source_gpg(options.gpg_homedir.as_ref())
            .arg("--armor")
            .arg("--export")
            .arg(&key)
            .output()
            .map_err(|e| format!("run gpg --export {}: {}", key, e))?;
        if !exported.status.success() || exported.stdout.is_empty() {
            return Err(format!("could not export public key '{}'", key));
        }

        let mut child = repo_gpg(&repo)
            .arg("--import")
            .stdin(Stdio::piped())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("run gpg --import: {}", e))?;

        child
            .stdin
            .as_mut()
            .ok_or_else(|| "could not open gpg import stdin".to_string())?
            .write_all(&exported.stdout)
            .map_err(|e| format!("write key to gpg import: {}", e))?;

        let status = child
            .wait()
            .map_err(|e| format!("wait for gpg import: {}", e))?;
        if !status.success() {
            return Err(format!("could not import public key '{}'", key));
        }

        println!("added recipient {}", key);
        imported.push(key);
    }

    Ok(imported)
}

fn source_gpg(homedir: Option<&PathBuf>) -> Command {
    let mut command = gpg_command();
    if let Some(homedir) = homedir {
        command.arg("--homedir").arg(homedir);
    }
    command
}

fn git_user_email() -> AppResult<String> {
    let output = Command::new("git")
        .arg("config")
        .arg("user.email")
        .output()
        .map_err(|e| format!("run git config user.email: {}", e))?;
    if !output.status.success() {
        return Err("could not read git config user.email".to_string());
    }

    let email = String::from_utf8(output.stdout)
        .map_err(|_| "git config user.email returned non-UTF-8 output".to_string())?
        .trim()
        .to_string();
    if email.is_empty() {
        return Err("git config user.email is empty".to_string());
    }

    Ok(email)
}

fn print_help() {
    println!(
        "git-secret tell - adds user(s) to the list of those able to encrypt/decrypt secrets.\n\
\n\
Usage:\n\
  git secret tell [-m] [-d <gpg-homedir>] [fingerprint-or-key-id-or-email]...\n\
\n\
Options:\n\
  -m  uses your current git config user.email setting as an identifier for the key\n\
  -d  specifies --homedir option for gpg\n\
  -h  shows this help"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tell_options_parse_git_email_and_homedir() {
        let options = Options::parse(vec![
            "-m".to_string(),
            "-d".to_string(),
            "keys".to_string(),
            "user@example.com".to_string(),
        ])
        .unwrap();

        assert!(options.use_git_email);
        assert_eq!(options.gpg_homedir, Some(PathBuf::from("keys")));
        assert_eq!(options.keys, vec!["user@example.com".to_string()]);
    }

    #[test]
    fn tell_options_parse_help() {
        let options = Options::parse(vec!["-h".to_string()]).unwrap();
        assert!(options.help);
    }

    #[test]
    fn tell_options_require_homedir_after_d() {
        assert!(Options::parse(vec!["-d".to_string()]).is_err());
    }
}
