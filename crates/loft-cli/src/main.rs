//! The `loft` command line (FG-9, FG-10). Exit codes: 0 on success, 1 on an error or a
//! failed check, 2 on a usage error (RQ-154). Every line is written through `out`, which a
//! closed standard output does not panic.
#![deny(clippy::print_stdout, clippy::print_stderr)]

mod check;
mod find;
mod fmt;
mod import;
mod init;
mod json;
mod merge_driver;
mod out;
mod publish;
mod reconcile;
mod render;
mod skill;
mod stop;
mod verify;
mod version;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use loftspec::lint::Severity;
use loftspec_store::merge_driver::LAUNCHER;

#[derive(Parser)]
#[command(
    name = "loft",
    about = "Author, check and format a Loft specification in git",
    after_help = "Exit codes: 0 on success, 1 on an error, a refusal or a failed check, 2 on a usage \
                  error.\nWith `--format json` a command prints one JSON document on standard \
                  output, described by its schema under `schemas/` in the Loft repository."
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
    /// Create an empty document in the current directory, and the `loft-spec` skill beside it
    /// (RQ-128).
    Init {
        /// Write only `spec/`, without the skill for coding agents.
        #[arg(long)]
        no_skill: bool,
        #[command(flatten)]
        output: Output,
    },
    /// Rewrite `spec/` in canonical layout, minting missing keys (RQ-140).
    Fmt {
        /// Report the files that would change, without writing (RQ-142).
        #[arg(long)]
        check: bool,
        #[command(flatten)]
        output: Output,
    },
    /// Read, validate and lint `spec/` (RQ-136).
    Check {
        /// Exit 1 on a finding at or above this severity (RQ-139).
        #[arg(long, value_name = "SEVERITY")]
        fail_on: Option<FailOn>,
        #[command(flatten)]
        output: Output,
    },
    /// Convert a Loft v1 export into `spec/` (RQ-130).
    Import {
        #[command(subcommand)]
        source: ImportSource,
    },
    /// Allocate labels and write the lock that pins the tree, on the publish branch (RQ-145).
    Publish {
        #[command(flatten)]
        output: Output,
    },
    /// Find the nodes and decisions a label, slug or key equals, or whose name contains the
    /// query, with the key a reference to each carries and where it is written.
    Find {
        /// A label such as `RQ-12`, a slug, a key, or part of a name.
        query: String,
        #[command(flatten)]
        output: Output,
    },
    /// Compare `loft.trace.yaml` with the tree and the tests that verify it, and accept the
    /// links a reviewer has checked.
    Reconcile {
        /// Exit 1 on a finding at or above this severity.
        #[arg(long, value_name = "SEVERITY")]
        fail_on: Option<FailOn>,
        /// Accept the link of this node, by key or label, once its tests are reviewed.
        #[arg(long, value_name = "NODE")]
        accept: Vec<String>,
        /// Accept every unreviewed or stale link whose tests are found.
        #[arg(long, conflicts_with = "accept")]
        accept_stale: bool,
        #[command(flatten)]
        output: Output,
    },
    /// Recompute the roots of `spec/` and compare them with the lock (RQ-148).
    Verify {
        #[command(flatten)]
        output: Output,
    },
    /// Merge one file of `spec/` as git's merge driver, writing the result to `<ours>` (RQ-150).
    MergeDriver {
        /// Register the driver in the repository instead of merging (RQ-152).
        #[arg(long, conflicts_with_all = ["base", "ours", "theirs", "path"])]
        install: bool,
        /// With `--install`, the shell command that starts `loft` where git runs the driver, such
        /// as `npx --no-install loft` for a dev dependency; `loft` unless given.
        #[arg(long, requires = "install", value_name = "COMMAND")]
        command: Option<String>,
        /// The common ancestor's version (git's `%O`).
        #[arg(required_unless_present = "install")]
        base: Option<PathBuf>,
        /// This side's version, which the merge overwrites (git's `%A`).
        #[arg(required_unless_present = "install")]
        ours: Option<PathBuf>,
        /// The other side's version (git's `%B`).
        #[arg(required_unless_present = "install")]
        theirs: Option<PathBuf>,
        /// The path of the file being merged, as git names it (git's `%P`).
        #[arg(required_unless_present = "install")]
        path: Option<String>,
        #[command(flatten)]
        output: Output,
    },
    /// Manage the `loft-spec` skill that teaches coding agents to write a specification.
    Skill {
        #[command(subcommand)]
        action: SkillAction,
    },
    /// Write the Markdown projection and the canonical JSON export (RQ-143).
    Render {
        /// Where to write the Markdown projection.
        #[arg(long, value_name = "PATH")]
        markdown: Option<PathBuf>,
        /// Where to write the canonical JSON export.
        #[arg(long, value_name = "PATH")]
        json: Option<PathBuf>,
        #[command(flatten)]
        output: Output,
    },
}

#[derive(Subcommand)]
enum SkillAction {
    /// Write the skill for Claude Code and Codex into the current directory, replacing any
    /// earlier version.
    Install {
        #[command(flatten)]
        output: Output,
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
        #[command(flatten)]
        output: Output,
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

/// The output a command prints, which every command takes.
#[derive(Args)]
struct Output {
    /// Print one JSON document on standard output (RQ-155).
    #[arg(long, value_name = "FORMAT")]
    format: Option<Format>,
}

impl Output {
    fn json(&self) -> bool {
        self.format == Some(Format::Json)
    }
}

fn main() -> ExitCode {
    let code = run(Cli::parse());
    out::finish(code)
}

/// Runs the command the arguments name and gives the code it exits with.
fn run(cli: Cli) -> ExitCode {
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
        Command::Init { no_skill, output } => init::run(root, !no_skill, output.json()),
        Command::Fmt { check, output } => fmt::run(root, check, output.json()),
        Command::Check { fail_on, output } => {
            check::run(root, fail_on.map(Severity::from), output.json())
        }
        Command::Import {
            source:
                ImportSource::V1 {
                    export,
                    counters,
                    output,
                },
        } => import::run(root, &export, counters.as_deref(), output.json()),
        Command::Publish { output } => publish::run(root, output.json()),
        Command::Find { query, output } => find::run(root, &query, output.json()),
        Command::Reconcile {
            fail_on,
            accept,
            accept_stale,
            output,
        } => {
            let options = reconcile::Options {
                fail_on: fail_on.map(Severity::from),
                accept: &accept,
                accept_stale,
            };
            reconcile::run(root, &options, output.json())
        }
        Command::Verify { output } => verify::run(root, output.json()),
        Command::MergeDriver {
            install: true,
            command,
            output,
            ..
        } => merge_driver::install(root, command.as_deref().unwrap_or(LAUNCHER), output.json()),
        Command::MergeDriver {
            base: Some(base),
            ours: Some(ours),
            theirs: Some(theirs),
            path: Some(path),
            output,
            ..
        } => merge_driver::run(
            root,
            &merge_driver::Inputs {
                base: &base,
                ours: &ours,
                theirs: &theirs,
                path: &path,
            },
            output.json(),
        ),
        // Clap requires the four paths unless `--install` is given.
        Command::MergeDriver { .. } => ExitCode::from(2),
        Command::Render {
            markdown,
            json,
            output,
        } => render::run(root, markdown, json, output.json()),
        Command::Skill {
            action: SkillAction::Install { output },
        } => skill::run(root, output.json()),
    }
}
