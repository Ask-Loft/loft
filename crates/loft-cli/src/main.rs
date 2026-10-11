//! The `loft` command line (D-11 FG-9, FG-10). Exit codes: 0 on success, 1 on an error or a
//! failed check, 2 on a usage error (RQ-154).

use std::path::Path;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use loftspec::codec::ReadError;
use loftspec_store::fs;

#[derive(Parser)]
#[command(
    name = "loft",
    about = "Author, check and format a Loft specification in git"
)]
#[command(arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Rewrite `spec/` in canonical layout, minting missing keys (RQ-140).
    Fmt {
        /// Report the files that would change, without writing (RQ-142).
        #[arg(long)]
        check: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Fmt { check } => fmt(Path::new("."), check),
    }
}

/// Prints each read error on its own line as `<path>:<line>:<column>: <message>` (RQ-158).
fn report(errors: &[ReadError]) {
    for error in errors {
        eprintln!("{error}");
    }
}

fn fmt(root: &Path, check: bool) -> ExitCode {
    let files = match fs::read_tree(root) {
        Ok(files) => files,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let formatted = match loftspec::fmt::format(&files) {
        Ok(formatted) => formatted,
        Err(errors) => {
            report(&errors);
            return ExitCode::FAILURE;
        }
    };
    for path in formatted.changed.keys() {
        println!("{path}");
    }
    if check {
        return if formatted.changed.is_empty() {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        };
    }
    match fs::write_changed(root, &formatted.changed) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
