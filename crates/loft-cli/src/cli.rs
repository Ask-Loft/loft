//! The arguments `loft` takes: every command, its options and the values they accept, as clap
//! parses them. A usage error exits 2 (RQ-154).

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use loftspec::lint::Severity;

use crate::{install, review, switch_command};

#[derive(Parser)]
#[command(
    name = "loft",
    about = "Author, check and format a Loft specification in git",
    after_help = "Exit codes: 0 on success, 1 on an error, a refusal or a failed check, 2 on a usage \
                  error.\nWith `--format json` a command prints one JSON document on standard \
                  output, described by its schema under `schemas/` in the Loft repository."
)]
#[command(arg_required_else_help = true, disable_version_flag = true)]
pub struct Cli {
    /// Print the tool version, the format and the hashing algorithm (RQ-157).
    #[arg(long)]
    pub version: bool,
    /// With `--version`, print one JSON document on standard output (AC-682).
    #[arg(long, value_name = "FORMAT", requires = "version")]
    pub format: Option<Format>,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Create an empty document in the current directory, and the `loft-spec` skill beside it
    /// (RQ-128).
    Init {
        /// Write only `spec/`, without the skill for coding agents.
        #[arg(long)]
        no_skill: bool,
        /// Write the skill without enabling the Claude Code plugin in `.claude/settings.json`.
        #[arg(long, conflicts_with = "no_skill")]
        no_plugin: bool,
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
        /// Exit 1 on a finding at or above this severity (RQ-139); with `--baseline`, on an
        /// introduced one.
        #[arg(long, value_name = "SEVERITY")]
        fail_on: Option<FailOn>,
        /// Mark each finding introduced or preexisting against the tree where HEAD left this
        /// revision, such as `develop`, and list the findings the tree resolved.
        #[arg(long, value_name = "REVISION")]
        baseline: Option<String>,
        #[command(flatten)]
        output: Output,
    },
    /// Convert a Loft v1 export (RQ-130) or the main specs of an `OpenSpec` project into `spec/`.
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
    /// List the names the specification may be using for one concept, and the subjects of each
    /// Product's statements, for the term list to decide.
    Terms {
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
        /// Accept every unreviewed, stale or changed link whose tests are found, and record the
        /// body of each located test that has none.
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
    /// Report the lock and whether the tree matches it, the repository's state, and the nodes
    /// added, changed and removed since HEAD left a base revision.
    Status {
        /// The revision whose merge base with HEAD the tree is compared with, such as `develop`.
        #[arg(long, value_name = "REVISION")]
        base: Option<String>,
        #[command(flatten)]
        output: Output,
    },
    /// Gather the context each assertion under review is read against, as a paged JSON packet,
    /// and judge nothing.
    Review(ReviewArgs),
    /// Merge one file of `spec/` as git's merge driver, writing the result to `<ours>` (RQ-150).
    MergeDriver(MergeDriverArgs),
    /// Manage the `loft-spec` skill that teaches coding agents to write a specification.
    Skill {
        #[command(subcommand)]
        action: SkillAction,
    },
    /// Record how this `loft` was installed, for the update notice and telemetry. The npm
    /// package runs it when it installs; run it yourself after installing another way.
    RecordInstall {
        /// How this `loft` was installed.
        way: install::Way,
        #[command(flatten)]
        output: Output,
    },
    /// Switch telemetry on or off for you on this machine, or report its state.
    Telemetry {
        #[command(subcommand)]
        action: SwitchAction,
    },
    /// Switch the check for a newer release on or off for you on this machine, or report its
    /// state.
    UpdateCheck {
        #[command(subcommand)]
        action: SwitchAction,
    },
    /// Write the Markdown projection, the canonical JSON export, or the `OpenSpec` projection
    /// (RQ-143).
    Render {
        /// Where to write the Markdown projection.
        #[arg(long, value_name = "PATH")]
        markdown: Option<PathBuf>,
        /// Where to write the canonical JSON export.
        #[arg(long, value_name = "PATH")]
        json: Option<PathBuf>,
        /// The directory to write the `OpenSpec` projection to: its main specs and a render map.
        #[arg(long, value_name = "DIR")]
        openspec: Option<PathBuf>,
        /// Replace the main specs already in the `OpenSpec` directory.
        #[arg(long, requires = "openspec")]
        force: bool,
        #[command(flatten)]
        output: Output,
    },
}

/// The arguments of `loft review`.
#[derive(Args)]
pub struct ReviewArgs {
    /// Review only what changed since HEAD left this revision, such as `develop`.
    #[arg(long, value_name = "REVISION")]
    pub base: Option<String>,
    /// Review only the assertions at or below the node with this key, slug or label.
    #[arg(long, value_name = "NODE")]
    pub node: Option<String>,
    /// The most characters of compact JSON a page's units take; a longer unit stands alone.
    #[arg(long, value_name = "N", default_value_t = 50_000)]
    pub max_chars: usize,
    /// The page to print, counted from 1.
    #[arg(long, value_name = "K", default_value_t = 1, value_parser = clap::value_parser!(u64).range(1..))]
    pub page: u64,
    #[command(flatten)]
    pub output: Output,
}

impl ReviewArgs {
    /// What the arguments ask `loft review` for.
    pub fn options(&self) -> review::Options<'_> {
        review::Options {
            base: self.base.as_deref(),
            node: self.node.as_deref(),
            max_chars: self.max_chars,
            page: usize::try_from(self.page).unwrap_or(usize::MAX),
        }
    }
}

/// The arguments of `loft merge-driver`: `--install`, or the four paths git passes a driver.
#[derive(Args)]
pub struct MergeDriverArgs {
    /// Register the driver in the repository instead of merging (RQ-152).
    #[arg(long, conflicts_with_all = ["base", "ours", "theirs", "path"])]
    pub install: bool,
    /// With `--install`, the shell command that starts `loft` where git runs the driver, such
    /// as `npx --no-install loft` for a dev dependency; `loft` unless given.
    #[arg(long, requires = "install", value_name = "COMMAND")]
    pub command: Option<String>,
    /// The common ancestor's version (git's `%O`).
    #[arg(required_unless_present = "install")]
    pub base: Option<PathBuf>,
    /// This side's version, which the merge overwrites (git's `%A`).
    #[arg(required_unless_present = "install")]
    pub ours: Option<PathBuf>,
    /// The other side's version (git's `%B`).
    #[arg(required_unless_present = "install")]
    pub theirs: Option<PathBuf>,
    /// The path of the file being merged, as git names it (git's `%P`).
    #[arg(required_unless_present = "install")]
    pub path: Option<String>,
    #[command(flatten)]
    pub output: Output,
}

#[derive(Subcommand)]
pub enum SkillAction {
    /// Write the skill for Claude Code and Codex into the current directory, replacing any
    /// earlier version.
    Install {
        /// Leave `.claude/settings.json` as it is, without enabling the Claude Code plugin.
        #[arg(long)]
        no_plugin: bool,
        #[command(flatten)]
        output: Output,
    },
}

#[derive(Subcommand)]
pub enum SwitchAction {
    /// Switch it on.
    On {
        #[command(flatten)]
        output: Output,
    },
    /// Switch it off.
    Off {
        #[command(flatten)]
        output: Output,
    },
    /// Report whether it is on, and which switch decided.
    Status {
        #[command(flatten)]
        output: Output,
    },
}

impl SwitchAction {
    pub fn split(&self) -> (switch_command::Action, bool) {
        match self {
            Self::On { output } => (switch_command::Action::On, output.json()),
            Self::Off { output } => (switch_command::Action::Off, output.json()),
            Self::Status { output } => (switch_command::Action::Status, output.json()),
        }
    }
}

#[derive(Subcommand)]
pub enum ImportSource {
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
    /// The main specs of an `OpenSpec` project.
    Openspec {
        /// The `OpenSpec` directory.
        #[arg(default_value = "openspec")]
        dir: PathBuf,
        /// The Product's name; by default, the name of the directory that holds `<DIR>`.
        #[arg(long, value_name = "NAME")]
        product: Option<String>,
        #[command(flatten)]
        output: Output,
    },
}

/// The severities `--fail-on` takes; any other is a usage error (AC-533).
#[derive(Clone, Copy, ValueEnum)]
pub enum FailOn {
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
pub enum Format {
    Json,
}

/// The output a command prints, which every command takes.
#[derive(Args)]
pub struct Output {
    /// Print one JSON document on standard output (RQ-155).
    #[arg(long, value_name = "FORMAT")]
    pub format: Option<Format>,
}

impl Output {
    pub fn json(&self) -> bool {
        self.format == Some(Format::Json)
    }
}

impl Command {
    /// The name the command event gives this command, if the command sends one; a command
    /// that sends one also checks for updates.
    pub const fn event_name(&self) -> Option<&'static str> {
        match self {
            Self::Init { .. } => Some("init"),
            Self::Fmt { .. } => Some("fmt"),
            Self::Check { .. } => Some("check"),
            Self::Import {
                source: ImportSource::V1 { .. },
            } => Some("import v1"),
            Self::Import {
                source: ImportSource::Openspec { .. },
            } => Some("import openspec"),
            Self::Publish { .. } => Some("publish"),
            Self::Reconcile { .. } => Some("reconcile"),
            Self::Terms { .. } => Some("terms"),
            Self::Find { .. } => Some("find"),
            Self::Review(..) => Some("review"),
            Self::Verify { .. } => Some("verify"),
            Self::Render { .. } => Some("render"),
            Self::Skill { .. } => Some("skill install"),
            Self::MergeDriver(..)
            | Self::Status { .. }
            | Self::RecordInstall { .. }
            | Self::Telemetry { .. }
            | Self::UpdateCheck { .. } => None,
        }
    }
}
