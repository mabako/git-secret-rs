# git-secret-rs

An early Rust implementation of the `git-secret` workflow.

This version keeps the same operating model as the bash implementation:

- tracked plaintext paths and SHA-256 sums are stored under `.gitsecret/paths/mapping.cfg`
- public keys live in a repository-local GPG home at `.gitsecret/keys`
- encrypted files are written next to their plaintext source as `<path>.secret`
- cryptographic operations are delegated to the local `gpg` executable

The binary is named `git-secret`, so Git can invoke it as `git secret ...` once it
is on `PATH`.

## Build

```powershell
cargo build
```

## Commands

```text
Usage: git-secret [COMMAND]

Commands:
  add           tells git secret which files hold secrets
  cat           prints the decrypted contents of the passed files
  changes       shows changes between the current versions of secret files and encrypted versions
  clean         deletes encrypted files in the current git-secret repo
  hide          writes an encrypted version of each file added by git-secret-add command
  init          initializes a git-secret repo by setting up its storage directory
  removeperson  removes public keys for passed email addresses or GPG fingerprints from repo’s git-secret keyring
  list          print the files currently considered secret in this repo
  remove        stops files from being tracked by git-secret
  reveal        decrypts passed files, or all files considered secret by git-secret
  tell          adds user(s) to the list of those able to encrypt/decrypt secrets
  whoknows      print email addresses allowed to access the secrets in this repo
  help          
  usage         
  version       

Options:
  -h, --help  Print help
```

## Environment variables

`git-secret-rs` supports the following environment variables:

| Variable | Default | Description |
| --- | --- | --- |
| `SECRETS_VERBOSE` | unset | Enables verbose mode for commands that support it, the same as passing `-v` |
| `SECRETS_GPG_COMMAND` | `gpg` | Sets the GPG command to run, such as `gpg`, `gpg2`, `pgp`, `/usr/local/gpg`, or another executable |
| `SECRETS_GPG_ARMOR` | `0` | Set to `1` to pass `--armor` to GPG and store encrypted secret files as text |
| `SECRETS_EXTENSION` | `.secret` | Sets the extension used for encrypted secret files |
| `SECRETS_DIR` | `.gitsecret` | Sets the directory where git-secret stores its files |
| `SECRETS_PINENTRY` | unset | Sets the value passed to GPG with `--pinentry-mode` |

When running under Git Bash with `MSYSTEM=MINGW64`, the default GPG command is resolved through `$ProgramFiles(x86)` to use the original GnuPG `gpg.exe`, unless `SECRETS_GPG_COMMAND` is set.

## Notes

This was 95% written by GPT-5.5, and is loosely based on the original git-secret's documentation (and experience with
having used it), along with a few bits (such as command-line flags and help) that was copied directly.

I haven't consulted the git-secret source code for all commands, and can't guarantee that all cases are handled the
same way that the original project handles them.
