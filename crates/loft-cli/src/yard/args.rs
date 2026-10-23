//! The arguments of `loft yard`: each command, and the options they share. An agent's command
//! names the agent that runs it; a command that acts on a branch's Yard Log names the page
//! server's port, and the write that opens Yard Log names the base branch it records. A usage
//! error exits 2 and names the argument at fault.

use std::path::PathBuf;

use clap::{Args, Subcommand};

use crate::cli::Output;

/// The port the page server listens on unless told another.
pub const DEFAULT_PORT: u16 = 7411;

/// The base branch Yard Log records where the write that opens it names none.
pub const DEFAULT_BASE: &str = "develop";

#[derive(Subcommand)]
pub enum YardAction {
    /// Read the branch's Yard Log: all of it, or what changed since a revision.
    Read(ReadArgs),
    /// Write a batch of blocks, read as JSON from `--input`, and send its questions, approvals
    /// and plans with `--send`.
    Write(WriteArgs),
    /// Withdraw an open submission.
    Withdraw(WithdrawArgs),
    /// Wait until a submission is answered or withdrawn, or Yard Log is closed, for at most
    /// `--timeout` seconds.
    Wait(WaitArgs),
    /// Close the branch's Yard Log once its work is finished.
    Close(CloseArgs),
    /// On the base branch, once `loft publish` is committed, record the publish in the Yard Log
    /// of the branch the merge brought in.
    Published(PublishedArgs),
    /// Record the person's answers and decisions given in the agent host's own dialog.
    Answer(AnswerArgs),
    /// Record that a session continues one that ended, as after `/clear` or a resume.
    Continue(ContinueArgs),
    /// Report the protocol the `loft yard` commands speak and the yardlog formats they read and
    /// write.
    Protocol {
        #[command(flatten)]
        output: Output,
    },
    /// Bring every open Yard Log under the tree's `.yard/` up to this `loft`'s format.
    Upgrade(UpgradeArgs),
    /// Run the page server on the loopback interface, unless one of this `loft`'s version or a
    /// newer one already serves the port; record the session as heard and its tree as served.
    Serve(ServeArgs),
}

#[derive(Args)]
pub struct ServeArgs {
    /// The port to serve the page on.
    #[arg(long, value_name = "PORT", default_value_t = DEFAULT_PORT)]
    pub port: u16,
    /// The session whose mod runs the command, recorded as heard.
    #[arg(long, value_name = "ID")]
    pub session: Option<String>,
    /// A directory of a worktree whose Yard Logs the page server is to serve.
    #[arg(long, value_name = "DIR")]
    pub tree: Option<PathBuf>,
    #[command(flatten)]
    pub output: Output,
}

/// The agent that runs a command, as a host names it.
#[derive(Args, Clone, Default)]
pub struct Runner {
    /// The id of the session whose agent runs the command.
    #[arg(long, value_name = "ID")]
    pub session: Option<String>,
    /// The id of the subagent that runs the command; the session's main agent where not given.
    #[arg(long, value_name = "ID", requires = "agent_type")]
    pub agent: Option<String>,
    /// The type of the subagent that runs the command.
    #[arg(long, value_name = "TYPE", requires = "agent")]
    pub agent_type: Option<String>,
}

/// The agent that runs a command that changes Yard Log, which must name its session.
#[derive(Args, Clone)]
pub struct Writer {
    /// The id of the session whose agent runs the command.
    #[arg(long, value_name = "ID", required = true)]
    pub session: String,
    /// The id of the subagent that runs the command; the session's main agent where not given.
    #[arg(long, value_name = "ID", requires = "agent_type")]
    pub agent: Option<String>,
    /// The type of the subagent that runs the command.
    #[arg(long, value_name = "TYPE", requires = "agent")]
    pub agent_type: Option<String>,
}

impl From<Writer> for Runner {
    fn from(writer: Writer) -> Self {
        Self {
            session: Some(writer.session),
            agent: writer.agent,
            agent_type: writer.agent_type,
        }
    }
}

/// Where a command finds the page server: its port, named in the result where none runs.
#[derive(Args, Clone)]
pub struct Beyond {
    /// The page server's port, named where no page server runs.
    #[arg(long, value_name = "PORT", default_value_t = DEFAULT_PORT)]
    pub port: u16,
}

#[derive(Args)]
pub struct ReadArgs {
    /// Return only the blocks written, answered or decided after this revision.
    #[arg(long, value_name = "REVISION")]
    pub since: Option<u64>,
    /// Return every version of each block returned, oldest first.
    #[arg(long)]
    pub versions: bool,
    /// Return each `loft` block's report, which is otherwise given by its size in bytes.
    #[arg(long)]
    pub reports: bool,
    /// Record the revision read as the one the calling agent last read: the agent's own read.
    #[arg(long, requires = "session")]
    pub mark_read: bool,
    #[command(flatten)]
    pub agent: Runner,
    #[command(flatten)]
    pub beyond: Beyond,
    #[command(flatten)]
    pub output: Output,
}

#[derive(Args)]
pub struct WriteArgs {
    /// The file that holds the batch as JSON, or `-` for standard input.
    #[arg(long, value_name = "FILE")]
    pub input: PathBuf,
    /// Open a submission of the batch's questions, approvals and plans.
    #[arg(long)]
    pub send: bool,
    /// The revision the batch is based on: a block of it changed since is refused.
    #[arg(long, value_name = "REVISION")]
    pub based_on: Option<u64>,
    /// The branch this one is merged into, recorded in Yard Log.s root where this write opens it.
    #[arg(long, value_name = "BRANCH", default_value = DEFAULT_BASE)]
    pub base: String,
    #[command(flatten)]
    pub writer: Writer,
    #[command(flatten)]
    pub beyond: Beyond,
    #[command(flatten)]
    pub output: Output,
}

#[derive(Args)]
pub struct WithdrawArgs {
    /// The key of the submission.
    #[arg(long, value_name = "KEY")]
    pub submission: String,
    #[command(flatten)]
    pub writer: Writer,
    #[command(flatten)]
    pub beyond: Beyond,
    #[command(flatten)]
    pub output: Output,
}

#[derive(Args)]
pub struct WaitArgs {
    /// The key of the submission.
    #[arg(long, value_name = "KEY")]
    pub submission: String,
    /// The most seconds to wait before returning `pending`; more than 540 is taken as 540.
    #[arg(long, value_name = "SECONDS", default_value_t = 540)]
    pub timeout: u64,
    #[command(flatten)]
    pub writer: Writer,
    #[command(flatten)]
    pub beyond: Beyond,
    #[command(flatten)]
    pub output: Output,
}

#[derive(Args)]
pub struct CloseArgs {
    #[command(flatten)]
    pub writer: Writer,
    #[command(flatten)]
    pub beyond: Beyond,
    #[command(flatten)]
    pub output: Output,
}

#[derive(Args)]
pub struct PublishedArgs {
    /// The branch whose Yard Log the merge brought into the base branch.
    #[arg(long, value_name = "BRANCH")]
    pub branch: String,
    /// A note on the publish, in Markdown.
    #[arg(long, value_name = "TEXT")]
    pub note: Option<String>,
    #[command(flatten)]
    pub writer: Writer,
    #[command(flatten)]
    pub beyond: Beyond,
    #[command(flatten)]
    pub output: Output,
}

#[derive(Args)]
pub struct AnswerArgs {
    /// The file that holds the answers and decisions as JSON, or `-` for standard input.
    #[arg(long, value_name = "FILE")]
    pub input: PathBuf,
    #[command(flatten)]
    pub writer: Writer,
    #[command(flatten)]
    pub beyond: Beyond,
    #[command(flatten)]
    pub output: Output,
}

#[derive(Args)]
pub struct UpgradeArgs {
    #[command(flatten)]
    pub beyond: Beyond,
    #[command(flatten)]
    pub output: Output,
}

#[derive(Args)]
pub struct ContinueArgs {
    /// The id of the session that goes on.
    #[arg(long, value_name = "ID")]
    pub session: String,
    /// The id of the session that ended, which `--session` continues.
    #[arg(long, value_name = "ID")]
    pub from: String,
    #[command(flatten)]
    pub output: Output,
}
