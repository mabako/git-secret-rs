use std::env;

mod args;
mod commands;
mod crypto;
mod git;
mod mapping;
mod paths;
mod process;

pub(crate) type AppResult<T> = Result<T, String>;

fn main() {
    if let Err(error) = commands::run(env::args_os().skip(1).collect()) {
        eprintln!("git-secret: {}", error);
        std::process::exit(1);
    }
}
