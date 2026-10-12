//! The `loft` command line (D-11 FG-9, FG-10). Exit codes: 0 on success, 1 on an error or a
//! failed check, 2 on a usage error (RQ-154).

mod check;
mod import;
mod json;
mod merge_driver;
mod publish;
mod render;
mod verify;
mod version;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use loftspec::lint::Severity;
use loftspec_store::FileStore;

#[derive(Parser)]
#[command(
    name = "loft",
    about = "Author, check and format a Loft specification in git"
)]
#[command(arg_required_else_help = true, disable_version_flag = true)]
struct Cli {
    /// Print the tool version, the format and the hashing algorithm (RQ-157).
    #[arg(long)]
    version: bool,
    /// With `--version`, print one JSON document on standard output (AC-682).
    #[arg(long, value_name = "FORMAT", requires = "version")]
    format: Option<Format>,
    #[command(subcommand)]
    command: Option<Command>,
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
    /// Allocate labels and write the lock that pins the tree, on the publish branch (RQ-145).
    Publish {
        /// Print one JSON document on standard output (RQ-155).
        #[arg(long, value_name = "FORMAT")]
        format: Option<Format>,
    },
    /// Recompute the roots of `spec/` and compare them with the lock (RQ-148).
    Verify {
        /// Print one JSON document on standard output (RQ-155).
        #[arg(long, value_name = "FORMAT")]
        format: Option<Format>,
    },
    /// Merge one file of `spec/` as git's merge driver, writing the result to `<ours>` (RQ-150).
    MergeDriver {
        /// The common ancestor's version (git's `%O`).
        base: PathBuf,
        /// This side's version, which the merge overwrites (git's `%A`).
        ours: PathBuf,
        /// The other side's version (git's `%B`).
        theirs: PathBuf,
        /// The path of the file being merged, as git names it (git's `%P`).
        path: String,
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
    if cli.version {
        version::run(cli.format == Some(Format::Json));
        return ExitCode::SUCCESS;
    }
    let Some(command) = cli.command else {
        // Clap prints the usage for a bare `loft`; any other way to reach here is a usage error.
        let _ = Cli::command().print_help();
        return ExitCode::from(2);
    };
    let root = Path::new(".");
    match command {
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
        Command::Publish { format } => publish::run(root, format == Some(Format::Json)),
        Command::Verify { format } => verify::run(root, format == Some(Format::Json)),
        Command::MergeDriver {
            base,
            ours,
            theirs,
            path,
        } => merge_driver::run(
            root,
            &merge_driver::Inputs {
                base: &base,
                ours: &ours,
                theirs: &theirs,
                path: &path,
            },
        ),
        Command::Render {
            markdown,
            json,
            format,
        } => render::run(root, markdown, json, format == Some(Format::Json)),
    }
}

/// Prints each error on its own line; a read or validation error as
/// `<path>:<line>:<column>: <message>` (RQ-158).
fn report(errors: &[impl std::fmt::Display]) {
    for error in errors {
        eprintln!("{error}");
    }
}

fn fmt(root: &Path, check: bool) -> ExitCode {
    let opened = FileStore::open(root).and_then(|store| Ok((store.read_files()?, store)));
    let (files, store) = match opened {
        Ok(opened) => opened,
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
    match store.write_files(formatted.changed) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
