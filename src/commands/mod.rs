use std::ffi::OsString;

use crate::args::Args;
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
mod whoknows;

pub(crate) fn run(args: Vec<OsString>) -> AppResult<()> {
    let mut args = Args::new(args);
    let command = args.next_string().unwrap_or_else(|| "usage".to_string());

    match command.as_str() {
        "-h" | "--help" | "help" | "usage" => {
            print_usage();
            Ok(())
        }
        "-v" | "--version" | "version" => {
            println!("git-secret-rs {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "init" => init::run(),
        "tell" => tell::run(args.rest_strings()?).map(|_| ()),
        "whoknows" => whoknows::run(),
        "killperson" | "removeperson" => remove_person::run(args.rest_strings()?),
        "add" => add::run(args.rest_paths()?),
        "remove" => remove::run(args.rest_paths()?),
        "list" => list::run(),
        "hide" => hide::run(hide::Options::parse(args.rest_strings()?)?),
        "reveal" => reveal::run(reveal::Options::parse(args.rest_strings()?)?),
        "cat" => cat::run(args.rest_paths()?),
        "clean" => clean::run(args.rest_paths()?),
        "changes" => changes::run(),
        unknown => Err(format!(
            "unknown command '{}'; run 'git secret usage' for help",
            unknown
        )),
    }
}

fn print_usage() {
    println!(
        "git-secret-rs {}\n\
\n\
Usage:\n\
  git secret init\n\
  git secret tell <fingerprint-or-key-id-or-email>...\n\
  git secret whoknows\n\
  git secret removeperson <fingerprint-or-key-id-or-email>...\n\
  git secret add <file>...\n\
  git secret remove <file>...\n\
  git secret list\n\
  git secret hide [--force] [--delete] [file...]\n\
  git secret reveal [--force] [file...]\n\
  git secret cat <file>...\n\
  git secret clean [file...]\n\
  git secret changes",
        env!("CARGO_PKG_VERSION")
    );
}
