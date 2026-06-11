use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{Read, Write};
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
        let plaintext = repo.join(&normalized);
        if !plaintext.is_file() {
            return Err(format!("{} is not a file", normalized));
        }
        let digest = sha256_file(&plaintext)?;
        if mapping.insert_or_update(normalized.clone(), digest) {
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

    for path in Mapping::load(&repo)?.paths() {
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

    let mut mapping = Mapping::load(&repo)?;
    let paths = selected_paths(&mapping, options.paths)?;
    let mut mapping_changed = false;
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
        let digest = sha256_file(&input)?;
        if mapping.insert_or_update(path.clone(), digest) {
            mapping_changed = true;
        }

        if options.delete_plaintext {
            fs::remove_file(&input).map_err(|e| format!("remove {}: {}", input.display(), e))?;
        }

        println!("encrypted {}", path);
    }

    if mapping_changed {
        mapping.save(&repo)?;
    }

    Ok(())
}

fn command_reveal(options: RevealOptions) -> AppResult<()> {
    let repo = Repo::discover()?;
    ensure_initialized(&repo)?;
    let mapping = Mapping::load(&repo)?;
    let paths = selected_paths(&mapping, options.paths)?;

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
    let mapping = Mapping::load(&repo)?;
    let paths = selected_paths(&mapping, paths)?;

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

    for entry in mapping.entries {
        let plaintext = repo.join(&entry.path);
        let secret = encrypted_path(&repo, &entry.path);
        let status = file_change_status(&plaintext, &secret, &entry.sha256)?;
        if let Some(status) = status {
            println!("{}\t{}", status, entry.path);
            changed = true;
        }
    }

    if !changed {
        println!("no changes");
    }

    Ok(())
}

fn selected_paths(mapping: &Mapping, paths: Vec<PathBuf>) -> AppResult<Vec<String>> {
    if paths.is_empty() {
        return Ok(mapping.paths());
    }

    paths
        .into_iter()
        .map(|path| normalize_secret_path(&path))
        .collect()
}

fn file_change_status(
    plaintext: &Path,
    secret: &Path,
    stored_sha256: &str,
) -> AppResult<Option<&'static str>> {
    if !plaintext.exists() {
        return Ok(None);
    }
    if !secret.exists() {
        return Ok(Some("new"));
    }
    if stored_sha256.is_empty() || sha256_file(plaintext)? != stored_sha256 {
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

fn sha256_file(path: &Path) -> AppResult<String> {
    let mut file = fs::File::open(path).map_err(|e| format!("open {}: {}", path.display(), e))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 8192];

    loop {
        let bytes = file
            .read(&mut buffer)
            .map_err(|e| format!("read {}: {}", path.display(), e))?;
        if bytes == 0 {
            break;
        }
        hasher.update(&buffer[..bytes]);
    }

    Ok(hasher.finalize_hex())
}

struct Sha256 {
    state: [u32; 8],
    buffer: [u8; 64],
    buffer_len: usize,
    bit_len: u64,
}

impl Sha256 {
    fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            buffer: [0; 64],
            buffer_len: 0,
            bit_len: 0,
        }
    }

    fn update(&mut self, mut input: &[u8]) {
        self.bit_len = self.bit_len.wrapping_add((input.len() as u64) * 8);

        if self.buffer_len > 0 {
            let to_copy = (64 - self.buffer_len).min(input.len());
            self.buffer[self.buffer_len..self.buffer_len + to_copy]
                .copy_from_slice(&input[..to_copy]);
            self.buffer_len += to_copy;
            input = &input[to_copy..];

            if self.buffer_len == 64 {
                let block = self.buffer;
                self.compress(&block);
                self.buffer_len = 0;
            }
        }

        while input.len() >= 64 {
            self.compress(&input[..64]);
            input = &input[64..];
        }

        if !input.is_empty() {
            self.buffer[..input.len()].copy_from_slice(input);
            self.buffer_len = input.len();
        }
    }

    fn finalize_hex(mut self) -> String {
        self.buffer[self.buffer_len] = 0x80;
        self.buffer_len += 1;

        if self.buffer_len > 56 {
            for byte in &mut self.buffer[self.buffer_len..] {
                *byte = 0;
            }
            let block = self.buffer;
            self.compress(&block);
            self.buffer_len = 0;
        }

        for byte in &mut self.buffer[self.buffer_len..56] {
            *byte = 0;
        }
        self.buffer[56..64].copy_from_slice(&self.bit_len.to_be_bytes());
        let block = self.buffer;
        self.compress(&block);

        let mut output = String::with_capacity(64);
        for word in self.state {
            output.push_str(&format!("{:08x}", word));
        }
        output
    }

    fn compress(&mut self, block: &[u8]) {
        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
            0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
            0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
            0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
            0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
            0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2,
        ];

        let mut schedule = [0u32; 64];
        for index in 0..16 {
            let start = index * 4;
            schedule[index] = u32::from_be_bytes([
                block[start],
                block[start + 1],
                block[start + 2],
                block[start + 3],
            ]);
        }
        for index in 16..64 {
            let s0 = schedule[index - 15].rotate_right(7)
                ^ schedule[index - 15].rotate_right(18)
                ^ (schedule[index - 15] >> 3);
            let s1 = schedule[index - 2].rotate_right(17)
                ^ schedule[index - 2].rotate_right(19)
                ^ (schedule[index - 2] >> 10);
            schedule[index] = schedule[index - 16]
                .wrapping_add(s0)
                .wrapping_add(schedule[index - 7])
                .wrapping_add(s1);
        }

        let mut a = self.state[0];
        let mut b = self.state[1];
        let mut c = self.state[2];
        let mut d = self.state[3];
        let mut e = self.state[4];
        let mut f = self.state[5];
        let mut g = self.state[6];
        let mut h = self.state[7];

        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[index])
                .wrapping_add(schedule[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }
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

#[derive(Debug, PartialEq, Eq)]
struct MappingEntry {
    path: String,
    sha256: String,
}

struct Mapping {
    entries: Vec<MappingEntry>,
}

impl Mapping {
    fn load(repo: &Repo) -> AppResult<Self> {
        let path = repo.join(MAPPING_FILE);
        let content =
            fs::read_to_string(&path).map_err(|e| format!("read {}: {}", path.display(), e))?;

        let mut entries = Vec::new();
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let (path, sha256) = parse_mapping_line(trimmed)?;
            if !entries
                .iter()
                .any(|existing: &MappingEntry| existing.path == path)
            {
                entries.push(MappingEntry { path, sha256 });
            }
        }
        entries.sort_by(|left, right| left.path.cmp(&right.path));

        Ok(Self { entries })
    }

    fn insert_or_update(&mut self, path: String, sha256: String) -> bool {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.path == path) {
            if entry.sha256 == sha256 {
                return false;
            }

            entry.sha256 = sha256;
            return true;
        }

        self.entries.push(MappingEntry { path, sha256 });
        self.entries
            .sort_by(|left, right| left.path.cmp(&right.path));
        true
    }

    fn remove(&mut self, path: &str) -> bool {
        let old_len = self.entries.len();
        self.entries.retain(|entry| entry.path != path);
        old_len != self.entries.len()
    }

    fn paths(&self) -> Vec<String> {
        self.entries
            .iter()
            .map(|entry| entry.path.clone())
            .collect()
    }

    fn save(&self, repo: &Repo) -> AppResult<()> {
        let path = repo.join(MAPPING_FILE);
        let mut content = self
            .entries
            .iter()
            .map(|entry| format!("{}:{}", entry.path, entry.sha256))
            .collect::<Vec<_>>()
            .join("\n");
        if !content.is_empty() {
            content.push('\n');
        }

        fs::write(&path, content).map_err(|e| format!("write {}: {}", path.display(), e))
    }
}

fn parse_mapping_line(line: &str) -> AppResult<(String, String)> {
    if let Some((path, sha256)) = line.rsplit_once(':') {
        if path.is_empty() {
            return Err("mapping entry has an empty path".to_string());
        }
        if !sha256.is_empty() && !is_sha256_hex(sha256) {
            return Err(format!(
                "mapping entry for '{}' has an invalid sha256",
                path
            ));
        }

        Ok((path.to_string(), sha256.to_ascii_lowercase()))
    } else {
        Ok((line.to_string(), String::new()))
    }
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
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
        let mut mapping = Mapping {
            entries: Vec::new(),
        };
        assert!(mapping.insert_or_update("b.env".to_string(), "b".repeat(64)));
        assert!(mapping.insert_or_update("a.env".to_string(), "a".repeat(64)));
        assert!(!mapping.insert_or_update("a.env".to_string(), "a".repeat(64)));
        assert_eq!(
            mapping.entries,
            vec![
                MappingEntry {
                    path: "a.env".to_string(),
                    sha256: "a".repeat(64),
                },
                MappingEntry {
                    path: "b.env".to_string(),
                    sha256: "b".repeat(64),
                }
            ]
        );
    }

    #[test]
    fn mapping_entry_parses_sha256() {
        let digest = "81ade6f4f3c9f5d447f8b5b646da9ac9a2e6119cfde90504f156a8d93c8963a5";
        assert_eq!(
            parse_mapping_line(&format!("aaaa.txt:{}", digest)).unwrap(),
            ("aaaa.txt".to_string(), digest.to_string())
        );
    }

    #[test]
    fn mapping_entry_allows_legacy_path_only_lines() {
        assert_eq!(
            parse_mapping_line("legacy.txt").unwrap(),
            ("legacy.txt".to_string(), String::new())
        );
    }

    #[test]
    fn sha256_matches_known_vector() {
        let mut hasher = Sha256::new();
        hasher.update(b"abc");
        assert_eq!(
            hasher.finalize_hex(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
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
