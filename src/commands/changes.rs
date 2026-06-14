use std::fs;
use std::path::{Path, PathBuf};

use crate::git::{ensure_initialized, Repo};
use crate::gpg::{gpg_arg_path, user_gpg, UserGpgOptions};
use crate::mapping::Mapping;
use crate::paths::{encrypted_path, selected_paths};
use crate::AppResult;

#[derive(clap::Args)]
pub(crate) struct Options {
    #[command(flatten)]
    gpg: UserGpgOptions,
    #[arg(value_name = "filename")]
    paths: Vec<PathBuf>,
}

pub(crate) fn run(options: Options) -> AppResult<()> {
    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;
    let mapping = Mapping::load(&repo)?;
    let paths = selected_paths(&repo, &mapping, options.paths)?;

    for path in paths {
        print_changes(&options.gpg, &repo, &path)?;
    }

    Ok(())
}

fn print_changes(gpg: &UserGpgOptions, repo: &Repo, path: &str) -> AppResult<()> {
    let plaintext = repo.join(path);
    let secret = encrypted_path(repo, path);

    if !plaintext.is_file() {
        return Err(format!("file not found: {}", plaintext.display()));
    }
    if !secret.is_file() {
        return Err(format!("encrypted file not found: {}", secret.display()));
    }

    let plaintext = fs::read(plaintext)
        .map_err(|e| format!("read plaintext {}: {}", repo.join(path).display(), e))?;
    let decrypted = decrypt_secret(gpg, &secret)?;
    println!("changes in {}:", repo.join(path).display());

    if plaintext == decrypted {
        return Ok(());
    }

    print_unified_diff(&decrypted, &plaintext);
    Ok(())
}

fn decrypt_secret(gpg: &UserGpgOptions, secret: &Path) -> AppResult<Vec<u8>> {
    let output = user_gpg(gpg)
        .arg("--batch")
        .arg("--decrypt")
        .arg(gpg_arg_path(secret))
        .output()
        .map_err(|e| format!("decrypt {}: {}", secret.display(), e))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(format!(
            "decrypt {}: gpg exited with {}",
            secret.display(),
            output.status
        ))
    }
}

fn print_unified_diff(old: &[u8], new: &[u8]) {
    let old = diff_lines(old);
    let new = diff_lines(new);
    let prefix_len = common_prefix_len(&old, &new);
    let suffix_len = common_suffix_len(&old[prefix_len..], &new[prefix_len..]);
    let old_changed = &old[prefix_len..old.len() - suffix_len];
    let new_changed = &new[prefix_len..new.len() - suffix_len];

    println!("--- encrypted");
    println!("+++ plaintext");
    println!(
        "@@ -{},{} +{},{} @@",
        prefix_len + 1,
        old_changed.len().max(1),
        prefix_len + 1,
        new_changed.len().max(1)
    );

    if prefix_len > 0 {
        println!(" {}", old[prefix_len - 1]);
    }
    for line in old_changed {
        println!("-{}", line);
    }
    for line in new_changed {
        println!("+{}", line);
    }
}

fn diff_lines(bytes: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(bytes);
    let mut lines = text
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line).to_string())
        .collect::<Vec<_>>();
    if bytes.ends_with(b"\n") {
        lines.pop();
    }
    lines
}

fn common_prefix_len(left: &[String], right: &[String]) -> usize {
    left.iter()
        .zip(right)
        .take_while(|(left, right)| left == right)
        .count()
}

fn common_suffix_len(left: &[String], right: &[String]) -> usize {
    left.iter()
        .rev()
        .zip(right.iter().rev())
        .take_while(|(left, right)| left == right)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Args, FromArgMatches};
    use std::path::PathBuf;

    fn command() -> clap::Command {
        Options::augment_args(clap::Command::new("changes"))
    }

    #[test]
    fn changes_options_parse_homedir_password_and_paths() {
        let matches = command()
            .try_get_matches_from(["changes", "-d", "keys", "-p", "secret", "file.txt"])
            .unwrap();
        let options = Options::from_arg_matches(&matches).unwrap();

        assert_eq!(options.gpg.homedir, Some(PathBuf::from("keys")));
        assert_eq!(options.gpg.passphrase, Some("secret".to_string()));
        assert_eq!(options.paths, vec![PathBuf::from("file.txt")]);
    }

    #[test]
    fn changes_options_require_values() {
        assert!(command().try_get_matches_from(["changes", "-d"]).is_err());
        assert!(command().try_get_matches_from(["changes", "-p"]).is_err());
    }
}
