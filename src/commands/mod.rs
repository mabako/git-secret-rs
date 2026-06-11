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
mod textconv;
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
        "init" => init::run(init::Options::parse(args.rest_strings()?)?),
        "tell" => tell::run(tell::Options::parse(args.rest_strings()?)?).map(|_| ()),
        "whoknows" => whoknows::run(whoknows::Options::parse(args.rest_strings()?)?),
        "killperson" | "removeperson" => remove_person::run(args.rest_strings()?),
        "add" => add::run(add::Options::parse(args.rest_strings()?)?),
        "remove" => remove::run(args.rest_paths()?),
        "list" => list::run(list::Options::parse(args.rest_strings()?)?),
        "hide" => hide::run(hide::Options::parse(args.rest_strings()?)?),
        "reveal" => reveal::run(reveal::Options::parse(args.rest_strings()?)?),
        "cat" => cat::run(cat::Options::parse(args.rest_strings()?)?),
        "textconv" => textconv::run(textconv::Options::parse(args.rest_strings()?)?),
        "clean" => clean::run(clean::Options::parse(args.rest_strings()?)?),
        "changes" => changes::run(changes::Options::parse(args.rest_strings()?)?),
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
  git secret init [-h]\n\
  git secret tell [-m] [-d <gpg-homedir>] [fingerprint-or-key-id-or-email]...\n\
  git secret whoknows [-l|-h]\n\
  git secret removeperson <fingerprint-or-key-id-or-email>...\n\
  git secret add [-h] <file>...\n\
  git secret remove <file>...\n\
  git secret list [-h]\n\
  git secret hide [-c] [-F] [-P] [-d] [-m] [-h] [file...]\n\
  git secret reveal [-f] [-F] [-d <gpg-homedir>] [-v] [-p <password>] [-P] [-h] [file...]\n\
  git secret cat [-d <gpg-homedir>] [-p <password>] <file> [file...]\n\
  git secret clean [-v] [-h]\n\
  git secret changes [-d <gpg-homedir>] [-p <password>] [-h]",
        env!("CARGO_PKG_VERSION")
    );
}
