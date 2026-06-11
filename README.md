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
git secret init
git secret tell <fingerprint-or-key-id-or-email>...
git secret whoknows
git secret removeperson <fingerprint-or-key-id-or-email>...
git secret add <file>...
git secret remove <file>...
git secret list
git secret hide [--force] [--delete] [file...]
git secret reveal [--force] [file...]
git secret cat <file>...
git secret clean [file...]
git secret changes
```

## Notes

This is intentionally conservative. It does not implement every option exposed by
the mature bash project yet, but it has the core state layout and command flow in
place so compatibility work can proceed command by command.
