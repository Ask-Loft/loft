//! The `loft` command line (D-11 FG-9, FG-10). Exit codes: 0 on success, 1 on an error or a
//! failed check, 2 on a usage error (RQ-154).

mod check;
mod import;
mod json;
mod render;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use loftspec::codec::ReadError;
use loftspec::lint::Severity;
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
    /// Read, validate and lint `spec/` (RQ-136).
    Check {
        /// Exit 1 on a finding at or above this severity (RQ-139).
        #[arg(long, value_name = "SEVERITY")]
        fail_on: Option<FailOn>,
        /// Print one JSON document on standard output (RQ-155).
        #[arg(long, value_name = "FORMAT")]
        format: Option<Format>,
    },
    /// Convert a Loft v1 export into `spec/` (RQ-130).
    Import {
        #[command(subcommand)]
        source: ImportSource,
    },
    /// Write the Markdown projection and the canonical JSON export (RQ-143).
    Render {
        /// Where to write the Markdown projection.
        #[arg(long, value_name = "PATH")]
        markdown: Option<PathBuf>,
        /// Where to write the canonical JSON export.
        #[arg(long, value_name = "PATH")]
        json: Option<PathBuf>,
        /// Print one JSON document on standard output (RQ-155).
        #[arg(long, value_name = "FORMAT")]
        format: Option<Format>,
    },
}

#[derive(Subcommand)]
enum ImportSource {
    /// A Loft v1 Authored State export (`get_authored_state`).
    V1 {
        /// The export file.
        export: PathBuf,
        /// A YAML mapping of label prefix to next value (RQ-131).
        #[arg(long, value_name = "FILE")]
        counters: Option<PathBuf>,
        /// Print one JSON document on standard output (RQ-155).
        #[arg(long, value_name = "FORMAT")]
        format: Option<Format>,
    },
}

/// The severities `--fail-on` takes; any other is a usage error (AC-533).
#[derive(Clone, Copy, ValueEnum)]
enum FailOn {
    Warning,
    Info,
    Hint,
}

impl From<FailOn> for Severity {
    fn from(fail_on: FailOn) -> Self {
        match fail_on {
            FailOn::Warning => Self::Warning,
            FailOn::Info => Self::Info,
            FailOn::Hint => Self::Hint,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Format {
    Json,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let root = Path::new(".");
    match cli.command {
        Command::Fmt { check } => fmt(root, check),
        Command::Check { fail_on, format } => check::run(
            root,
            fail_on.map(Severity::from),
            format == Some(Format::Json),
        ),
        Command::Import {
            source:
                ImportSource::V1 {
                    export,
                    counters,
                    format,
                },
        } => import::run(
            root,
            &export,
            counters.as_deref(),
            format == Some(Format::Json),
        ),
        Command::Render {
            markdown,
            json,
            format,
        } => render::run(root, markdown, json, format == Some(Format::Json)),
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
