use std::ffi::OsString;

use clap::{CommandFactory, Parser, Subcommand};

use crate::AppResult;

mod add;
mod cat;
mod changes;
mod clean;
mod hide;
mod init;
mod list;
mod remove;
mod remove_person;
mod reveal;
mod tell;
mod textconv;
mod whoknows;

const SECRETS_VERBOSE_ENV: &str = "SECRETS_VERBOSE";
const SECRETS_GPG_ARMOR_ENV: &str = "SECRETS_GPG_ARMOR";

pub(crate) fn run(args: Vec<OsString>) -> AppResult<()> {
    let cli = match Cli::try_parse_from(std::iter::once(OsString::from("git-secret")).chain(args)) {
        Ok(cli) => cli,
        Err(error) if error.use_stderr() => return Err(error.to_string()),
        Err(error) => {
            print!("{}", error);
            return Ok(());
        }
    };

    if matches!(cli.command, None | Some(Command::Help | Command::Usage)) {
        Cli::command()
            .print_help()
            .map_err(|e| format!("print help: {}", e))?;
        println!();
        return Ok(());
    }
    if matches!(cli.command, Some(Command::Version)) {
        println!("git-secret-rs {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    match cli.command.expect("command should be handled above") {
        Command::Init(options) => init::run(options),
        Command::Tell(options) => tell::run(options).map(|_| ()),
        Command::WhoKnows(options) => whoknows::run(options),
        Command::RemovePerson(options) => remove_person::run(options),
        Command::Add(options) => add::run(options),
        Command::Remove(options) => remove::run(options),
        Command::List(options) => list::run(options),
        Command::Hide(options) => hide::run(options),
        Command::Reveal(options) => reveal::run(options),
        Command::Cat(options) => cat::run(options),
        Command::Textconv(options) => textconv::run(options),
        Command::Clean(options) => clean::run(options),
        Command::Changes(options) => changes::run(options),
        Command::Help | Command::Usage | Command::Version => unreachable!(),
    }
}

pub(crate) fn secrets_verbose() -> bool {
    std::env::var_os(SECRETS_VERBOSE_ENV).is_some()
}

pub(crate) fn secrets_gpg_armor() -> bool {
    std::env::var_os(SECRETS_GPG_ARMOR_ENV).is_some_and(|value| value == "1")
}

#[derive(Parser)]
#[command(disable_help_subcommand = true, disable_version_flag = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// tells git secret which files hold secrets.
    Add(add::Options),

    /// prints the decrypted contents of the passed files.
    Cat(cat::Options),

    /// shows changes between the current versions of secret files and encrypted versions.
    Changes(changes::Options),

    /// deletes encrypted files in the current git-secret repo.
    Clean(clean::Options),

    /// writes an encrypted version of each file added by git-secret-add command.
    Hide(hide::Options),

    /// initializes a git-secret repo by setting up its storage directory.
    Init(init::Options),

    /// removes public keys for passed email addresses or GPG fingerprints from repo’s git-secret keyring.
    #[command(name = "removeperson", alias = "killperson")]
    RemovePerson(remove_person::Options),

    /// print the files currently considered secret in this repo.
    List(list::Options),

    /// stops files from being tracked by git-secret.
    Remove(remove::Options),

    /// decrypts passed files, or all files considered secret by git-secret.
    Reveal(reveal::Options),

    /// adds user(s) to the list of those able to encrypt/decrypt secrets.
    Tell(tell::Options),

    /// print email addresses allowed to access the secrets in this repo.
    #[command(name = "whoknows")]
    WhoKnows(whoknows::Options),

    #[command(hide = true)]
    Textconv(textconv::Options),

    Help,
    Usage,
    Version,
}
