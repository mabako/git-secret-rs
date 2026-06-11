use std::ffi::OsString;

#[cfg(test)]
use clap::{Args as ClapArgs, FromArgMatches};
use clap::{Parser, Subcommand};

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

pub(crate) fn run(args: Vec<OsString>) -> AppResult<()> {
    let cli = Cli::try_parse_from(std::iter::once(OsString::from("git-secret")).chain(args))
        .map_err(|error| error.to_string())?;

    if cli.help || matches!(cli.command, None | Some(Command::Help | Command::Usage)) {
        print_usage();
        return Ok(());
    }
    if cli.version || matches!(cli.command, Some(Command::Version)) {
        println!("git-secret-rs {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    match cli.command.expect("command should be handled above") {
        Command::Init(options) => init::run(options),
        Command::Tell(options) => tell::run(options).map(|_| ()),
        Command::Whoknows(options) => whoknows::run(options),
        Command::Removeperson(options) => remove_person::run(options),
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

#[derive(Parser)]
#[command(
    disable_help_flag = true,
    disable_help_subcommand = true,
    disable_version_flag = true
)]
struct Cli {
    #[arg(short = 'h', long = "help")]
    help: bool,
    #[arg(short = 'v', long = "version")]
    version: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    Init(init::Options),
    Tell(tell::Options),
    Whoknows(whoknows::Options),
    #[command(alias = "killperson")]
    Removeperson(remove_person::Options),
    Add(add::Options),
    Remove(remove::Options),
    List(list::Options),
    Hide(hide::Options),
    Reveal(reveal::Options),
    Cat(cat::Options),
    Textconv(textconv::Options),
    Clean(clean::Options),
    Changes(changes::Options),
    Help,
    Usage,
    Version,
}

#[cfg(test)]
pub(crate) fn parse_options<T>(name: &'static str, args: Vec<String>) -> AppResult<T>
where
    T: ClapArgs + FromArgMatches,
{
    let command = T::augment_args(clap::Command::new(name).disable_help_flag(true));
    let matches = command
        .try_get_matches_from(std::iter::once(name.to_string()).chain(args))
        .map_err(|error| error.to_string())?;
    T::from_arg_matches(&matches).map_err(|error| error.to_string())
}

fn print_usage() {
    println!(
        "git-secret-rs {}\n\
\n\
Usage:\n\
  git secret init [-h]\n\
  git secret tell [-m] [-d <gpg-homedir>] [fingerprint-or-key-id-or-email]...\n\
  git secret whoknows [-l|-h]\n\
  git secret removeperson [-h] <fingerprint-or-key-id-or-email>...\n\
  git secret add [-h] <file>...\n\
  git secret remove [-c] [-h] <file>...\n\
  git secret list [-h]\n\
  git secret hide [-c] [-F] [-P] [-d] [-m] [-h] [file...]\n\
  git secret reveal [-f] [-F] [-d <gpg-homedir>] [-v] [-p <password>] [-P] [-h] [file...]\n\
  git secret cat [-d <gpg-homedir>] [-p <password>] <file> [file...]\n\
  git secret clean [-v] [-h]\n\
  git secret changes [-d <gpg-homedir>] [-p <password>] [-h]",
        env!("CARGO_PKG_VERSION")
    );
}
