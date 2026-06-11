use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

type AppResult<T> = Result<T, String>;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const SECRET_DIR: &str = ".gitsecret";
const KEYS_DIR: &str = ".gitsecret/keys";
const PATHS_DIR: &str = ".gitsecret/paths";
const MAPPING_FILE: &str = ".gitsecret/paths/mapping.cfg";

fn main() {
    if let Err(error) = run(env::args_os().skip(1).collect()) {
        eprintln!("git-secret: {}", error);
        std::process::exit(1);
    }
}

fn run(args: Vec<OsString>) -> AppResult<()> {
    let mut args = Args::new(args);
    let command = args.next_string().unwrap_or_else(|| "usage".to_string());

    match command.as_str() {
        "-h" | "--help" | "help" | "usage" => {
            print_usage();
            Ok(())
        }
        "-v" | "--version" | "version" => {
            println!("git-secret-rs {}", VERSION);
            Ok(())
        }
        "init" => command_init(),
        "tell" => command_tell(args.rest_strings()?).map(|_| ()),
        "whoknows" => command_whoknows(),
        "killperson" | "removeperson" => command_remove_person(args.rest_strings()?),
        "add" => command_add(args.rest_paths()?),
        "remove" => command_remove(args.rest_paths()?),
        "list" => command_list(),
        "hide" => command_hide(HideOptions::parse(args.rest_strings()?)?),
        "reveal" => command_reveal(RevealOptions::parse(args.rest_strings()?)?),
        "cat" => command_cat(args.rest_paths()?),
        "clean" => command_clean(args.rest_paths()?),
        "changes" => command_changes(),
        unknown => Err(format!(
            "unknown command '{}'; run 'git secret usage' for help",
            unknown
        )),
    }
}

fn command_init() -> AppResult<()> {
    let repo = Repo::discover()?;
    fs::create_dir_all(repo.join(KEYS_DIR)).map_err(|e| format!("create {}: {}", KEYS_DIR, e))?;
    fs::create_dir_all(repo.join(PATHS_DIR)).map_err(|e| format!("create {}: {}", PATHS_DIR, e))?;

    let mapping = repo.join(MAPPING_FILE);
    if !mapping.exists() {
        fs::write(&mapping, "").map_err(|e| format!("write {}: {}", mapping.display(), e))?;
    }

    let key_gitignore = repo.join(KEYS_DIR).join(".gitignore");
    if !key_gitignore.exists() {
        fs::write(
            &key_gitignore,
            "random_seed\ntrustdb.gpg\nS.gpg-agent*\nprivate-keys-v1.d/\n",
        )
        .map_err(|e| format!("write {}: {}", key_gitignore.display(), e))?;
    }

    gpg(&repo)
        .arg("--list-keys")
        .status_ok("initialize repository keyring")?;

    println!("created {}", repo.join(SECRET_DIR).display());
    Ok(())
}

fn command_tell(keys: Vec<String>) -> AppResult<Vec<String>> {
    if keys.is_empty() {
        return Err("tell requires at least one key id or email".to_string());
    }

    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;
    let mut imported = Vec::new();

    for key in keys {
        let exported = Command::new("gpg")
            .arg("--armor")
            .arg("--export")
            .arg(&key)
            .output()
            .map_err(|e| format!("run gpg --export {}: {}", key, e))?;
        if !exported.status.success() || exported.stdout.is_empty() {
            return Err(format!("could not export public key '{}'", key));
        }

        let mut child = gpg(&repo)
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

fn command_whoknows() -> AppResult<()> {
    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;
    let recipients = recipient_ids(&repo)?;

    if recipients.is_empty() {
        println!("no recipients configured");
        return Ok(());
    }

    for recipient in recipients {
        println!("{}", recipient);
    }

    Ok(())
}

fn command_remove_person(keys: Vec<String>) -> AppResult<()> {
    if keys.is_empty() {
        return Err("removeperson requires at least one key id or email".to_string());
    }

    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;

    for key in keys {
        gpg(&repo)
            .arg("--batch")
            .arg("--yes")
            .arg("--delete-keys")
            .arg(&key)
            .status_ok(&format!("remove recipient {}", key))?;
        println!("removed recipient {}", key);
    }

    Ok(())
}

fn command_add(paths: Vec<PathBuf>) -> AppResult<()> {
    if paths.is_empty() {
        return Err("add requires at least one file".to_string());
    }

    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;
    let mut mapping = Mapping::load(&repo)?;
    let mut added = 0;

    for path in paths {
        let normalized = normalize_secret_path(&path)?;
        if !repo.join(&normalized).is_file() {
            return Err(format!("{} is not a file", normalized));
        }
        if mapping.insert(normalized.clone()) {
            println!("added {}", normalized);
            added += 1;
        }
    }

    if added > 0 {
        mapping.save(&repo)?;
    }

    Ok(())
}

fn command_remove(paths: Vec<PathBuf>) -> AppResult<()> {
    if paths.is_empty() {
        return Err("remove requires at least one file".to_string());
    }

    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;
    let mut mapping = Mapping::load(&repo)?;
    let mut removed = 0;

    for path in paths {
        let normalized = normalize_secret_path(&path)?;
        if mapping.remove(&normalized) {
            println!("removed {}", normalized);
            removed += 1;
        }
    }

    if removed > 0 {
        mapping.save(&repo)?;
    }

    Ok(())
}

fn command_list() -> AppResult<()> {
    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;

    for path in Mapping::load(&repo)?.paths {
        println!("{}", path);
    }

    Ok(())
}

fn command_hide(options: HideOptions) -> AppResult<()> {
    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;
    let recipients = recipient_key_ids(&repo)?;
    if recipients.is_empty() {
        return Err("no recipients configured; run 'git secret tell <key>' first".to_string());
    }

    let paths = selected_paths(&repo, options.paths)?;
    for path in paths {
        let input = repo.join(&path);
        let output = encrypted_path(&repo, &path);

        if !input.is_file() {
            return Err(format!("{} is not a file", path));
        }
        if output.exists() && !options.force {
            return Err(format!(
                "{} already exists; pass --force to overwrite",
                output.display()
            ));
        }

        let mut cmd = gpg(&repo);
        cmd.arg("--batch")
            .arg("--yes")
            .arg("--trust-model")
            .arg("always")
            .arg("--encrypt");
        for recipient in &recipients {
            cmd.arg("--recipient").arg(recipient);
        }
        cmd.arg("--output").arg(&output).arg(&input);
        cmd.status_ok(&format!("encrypt {}", path))?;

        if options.delete_plaintext {
            fs::remove_file(&input).map_err(|e| format!("remove {}: {}", input.display(), e))?;
        }

        println!("encrypted {}", path);
    }

    Ok(())
}

fn command_reveal(options: RevealOptions) -> AppResult<()> {
    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;
    let paths = selected_paths(&repo, options.paths)?;

    for path in paths {
        let input = encrypted_path(&repo, &path);
        let output = repo.join(&path);

        if !input.is_file() {
            return Err(format!("{} does not exist", input.display()));
        }
        if output.exists() && !options.force {
            return Err(format!(
                "{} already exists; pass --force to overwrite",
                output.display()
            ));
        }

        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("create {}: {}", parent.display(), e))?;
        }

        gpg(&repo)
            .arg("--batch")
            .arg("--yes")
            .arg("--decrypt")
            .arg("--output")
            .arg(&output)
            .arg(&input)
            .status_ok(&format!("decrypt {}", path))?;
        println!("decrypted {}", path);
    }

    Ok(())
}

fn command_cat(paths: Vec<PathBuf>) -> AppResult<()> {
    if paths.is_empty() {
        return Err("cat requires at least one file".to_string());
    }

    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;

    for path in paths {
        let normalized = normalize_secret_path(&path)?;
        let secret = encrypted_path(&repo, &normalized);
        gpg(&repo)
            .arg("--batch")
            .arg("--decrypt")
            .arg(&secret)
            .status_ok(&format!("decrypt {}", normalized))?;
    }

    Ok(())
}

fn command_clean(paths: Vec<PathBuf>) -> AppResult<()> {
    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;
    let paths = selected_paths(&repo, paths)?;

    for path in paths {
        let plaintext = repo.join(&path);
        if plaintext.exists() {
            fs::remove_file(&plaintext)
                .map_err(|e| format!("remove {}: {}", plaintext.display(), e))?;
            println!("removed {}", path);
        }
    }

    Ok(())
}

fn command_changes() -> AppResult<()> {
    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;
    let mapping = Mapping::load(&repo)?;
    let mut changed = false;

    for path in mapping.paths {
        let plaintext = repo.join(&path);
        let secret = encrypted_path(&repo, &path);
        let status = file_change_status(&plaintext, &secret)?;
        if let Some(status) = status {
            println!("{}\t{}", status, path);
            changed = true;
        }
    }

    if !changed {
        println!("no changes");
    }

    Ok(())
}

fn selected_paths(repo: &Repo, paths: Vec<PathBuf>) -> AppResult<Vec<String>> {
    if paths.is_empty() {
        return Ok(Mapping::load(repo)?.paths);
    }

    paths
        .into_iter()
        .map(|path| normalize_secret_path(&path))
        .collect()
}

fn file_change_status(plaintext: &Path, secret: &Path) -> AppResult<Option<&'static str>> {
    if !plaintext.exists() {
        return Ok(None);
    }
    if !secret.exists() {
        return Ok(Some("new"));
    }

    let plain_modified = plaintext
        .metadata()
        .and_then(|m| m.modified())
        .map_err(|e| format!("read {} metadata: {}", plaintext.display(), e))?;
    let secret_modified = secret
        .metadata()
        .and_then(|m| m.modified())
        .map_err(|e| format!("read {} metadata: {}", secret.display(), e))?;

    if plain_modified > secret_modified {
        Ok(Some("modified"))
    } else {
        Ok(None)
    }
}

fn ensure_initialized(repo: &Repo) -> AppResult<()> {
    if !repo.join(MAPPING_FILE).is_file() {
        return Err("repository is not initialized; run 'git secret init' first".to_string());
    }

    Ok(())
}

fn encrypted_path(repo: &Repo, path: &str) -> PathBuf {
    repo.join(format!("{}.secret", path))
}

fn recipient_key_ids(repo: &Repo) -> AppResult<Vec<String>> {
    let output = gpg(repo)
        .arg("--with-colons")
        .arg("--list-keys")
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

fn recipient_ids(repo: &Repo) -> AppResult<Vec<String>> {
    let output = gpg(repo)
        .arg("--with-colons")
        .arg("--list-keys")
        .output()
        .map_err(|e| format!("list recipients: {}", e))?;
    if !output.status.success() {
        return Err("could not list repository recipients".to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut recipients = Vec::new();
    for line in stdout.lines() {
        let fields: Vec<&str> = line.split(':').collect();
        if fields.first() == Some(&"uid") {
            if let Some(uid) = fields.get(9) {
                if !uid.is_empty() {
                    recipients.push((*uid).to_string());
                }
            }
        }
    }

    Ok(recipients)
}

fn normalize_secret_path(path: &Path) -> AppResult<String> {
    if path.is_absolute() {
        return Err(format!(
            "{} must be relative to the repository",
            path.display()
        ));
    }

    let mut pieces = Vec::new();
    for component in path.components() {
        use std::path::Component::*;
        match component {
            CurDir => {}
            Normal(piece) => pieces.push(os_to_string(piece)?),
            ParentDir => return Err(format!("{} must not contain '..'", path.display())),
            RootDir | Prefix(_) => {
                return Err(format!(
                    "{} must be relative to the repository",
                    path.display()
                ))
            }
        }
    }

    if pieces.is_empty() {
        return Err("empty file path".to_string());
    }

    let normalized = pieces.join("/");
    if normalized.ends_with(".secret") {
        return Err("add the plaintext path, not the .secret file".to_string());
    }

    Ok(normalized)
}

fn os_to_string(value: &OsStr) -> AppResult<String> {
    value
        .to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "path is not valid UTF-8".to_string())
}

fn gpg(repo: &Repo) -> Command {
    let mut command = Command::new("gpg");
    command.arg("--homedir").arg(repo.join(KEYS_DIR));
    command
}

fn print_usage() {
    println!(
        "git-secret-rs {}\n\
\n\
Usage:\n\
  git secret init\n\
  git secret tell <key-id-or-email>...\n\
  git secret whoknows\n\
  git secret removeperson <key-id-or-email>...\n\
  git secret add <file>...\n\
  git secret remove <file>...\n\
  git secret list\n\
  git secret hide [--force] [--delete] [file...]\n\
  git secret reveal [--force] [file...]\n\
  git secret cat <file>...\n\
  git secret clean [file...]\n\
  git secret changes",
        VERSION
    );
}

struct Repo {
    root: PathBuf,
}

impl Repo {
    fn discover() -> AppResult<Self> {
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

    fn join<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        self.root.join(path)
    }
}

struct Mapping {
    paths: Vec<String>,
}

impl Mapping {
    fn load(repo: &Repo) -> AppResult<Self> {
        let path = repo.join(MAPPING_FILE);
        let content =
            fs::read_to_string(&path).map_err(|e| format!("read {}: {}", path.display(), e))?;

        let mut paths = Vec::new();
        for line in content.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !paths.iter().any(|existing| existing == trimmed) {
                paths.push(trimmed.to_string());
            }
        }
        paths.sort();

        Ok(Self { paths })
    }

    fn insert(&mut self, path: String) -> bool {
        if self.paths.iter().any(|existing| existing == &path) {
            return false;
        }

        self.paths.push(path);
        self.paths.sort();
        true
    }

    fn remove(&mut self, path: &str) -> bool {
        let old_len = self.paths.len();
        self.paths.retain(|existing| existing != path);
        old_len != self.paths.len()
    }

    fn save(&self, repo: &Repo) -> AppResult<()> {
        let path = repo.join(MAPPING_FILE);
        let mut content = self.paths.join("\n");
        if !content.is_empty() {
            content.push('\n');
        }

        fs::write(&path, content).map_err(|e| format!("write {}: {}", path.display(), e))
    }
}

struct HideOptions {
    force: bool,
    delete_plaintext: bool,
    paths: Vec<PathBuf>,
}

impl HideOptions {
    fn parse(args: Vec<String>) -> AppResult<Self> {
        let mut force = false;
        let mut delete_plaintext = false;
        let mut paths = Vec::new();

        for arg in args {
            match arg.as_str() {
                "-f" | "--force" => force = true,
                "-d" | "--delete" => delete_plaintext = true,
                _ if arg.starts_with('-') => return Err(format!("unknown hide option '{}'", arg)),
                _ => paths.push(PathBuf::from(arg)),
            }
        }

        Ok(Self {
            force,
            delete_plaintext,
            paths,
        })
    }
}

struct RevealOptions {
    force: bool,
    paths: Vec<PathBuf>,
}

impl RevealOptions {
    fn parse(args: Vec<String>) -> AppResult<Self> {
        let mut force = false;
        let mut paths = Vec::new();

        for arg in args {
            match arg.as_str() {
                "-f" | "--force" => force = true,
                _ if arg.starts_with('-') => {
                    return Err(format!("unknown reveal option '{}'", arg))
                }
                _ => paths.push(PathBuf::from(arg)),
            }
        }

        Ok(Self { force, paths })
    }
}

struct Args {
    values: std::vec::IntoIter<OsString>,
}

impl Args {
    fn new(values: Vec<OsString>) -> Self {
        Self {
            values: values.into_iter(),
        }
    }

    fn next_string(&mut self) -> Option<String> {
        self.values
            .next()
            .map(|value| value.to_string_lossy().into())
    }

    fn rest_strings(self) -> AppResult<Vec<String>> {
        self.values
            .map(|value| {
                value
                    .into_string()
                    .map_err(|_| "argument is not valid UTF-8".to_string())
            })
            .collect()
    }

    fn rest_paths(self) -> AppResult<Vec<PathBuf>> {
        self.values
            .map(|value| {
                value
                    .into_string()
                    .map(PathBuf::from)
                    .map_err(|_| "argument is not valid UTF-8".to_string())
            })
            .collect()
    }
}

trait CommandExt {
    fn status_ok(&mut self, action: &str) -> AppResult<()>;
}

impl CommandExt for Command {
    fn status_ok(&mut self, action: &str) -> AppResult<()> {
        let status = self
            .status()
            .map_err(|e| format!("{}: failed to run command: {}", action, e))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("{}: command exited with {}", action, status))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_plain_relative_paths() {
        assert_eq!(
            normalize_secret_path(Path::new("./config/secrets.env")).unwrap(),
            "config/secrets.env"
        );
    }

    #[test]
    fn reject_parent_paths() {
        assert!(normalize_secret_path(Path::new("../secrets.env")).is_err());
        assert!(normalize_secret_path(Path::new("config/../secrets.env")).is_err());
    }

    #[test]
    fn reject_secret_suffix() {
        assert!(normalize_secret_path(Path::new("secrets.env.secret")).is_err());
    }

    #[test]
    fn mapping_stays_sorted_and_unique() {
        let mut mapping = Mapping { paths: Vec::new() };
        assert!(mapping.insert("b.env".to_string()));
        assert!(mapping.insert("a.env".to_string()));
        assert!(!mapping.insert("a.env".to_string()));
        assert_eq!(
            mapping.paths,
            vec!["a.env".to_string(), "b.env".to_string()]
        );
    }

    #[test]
    fn hide_options_parse_flags_and_paths() {
        let options = HideOptions::parse(vec![
            "--force".to_string(),
            "--delete".to_string(),
            "secret.env".to_string(),
        ])
        .unwrap();

        assert!(options.force);
        assert!(options.delete_plaintext);
        assert_eq!(options.paths, vec![PathBuf::from("secret.env")]);
    }
}
