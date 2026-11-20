//! The arguments of `loft interface`.

use std::num::NonZeroU64;

use clap::Subcommand;

use super::Output;
use crate::interface;

#[derive(Subcommand)]
pub enum InterfaceAction {
    /// Pin a copy of another document's Interface, named as `<handle>:<label>` or
    /// `<handle>:<key>`, as that document published it at its remote's head.
    Add {
        /// The Interface, as its handle and its label or key.
        interface: String,
        /// The publish sequence of the other document's state to take the copy from, in place of
        /// its head.
        #[arg(long, value_name = "SEQUENCE")]
        at: Option<NonZeroU64>,
        /// The address of the other document's remote, which lists it under the handle the
        /// first time.
        #[arg(long, value_name = "ADDRESS")]
        remote: Option<String>,
        /// Print the address to open for a sign-in, without opening a browser.
        #[arg(long)]
        no_browser: bool,
        #[command(flatten)]
        output: Output,
    },
    /// Move the pins of the copies to their remotes' heads and report how each promise changed:
    /// every copy, the copies under a handle, or one copy named as `<handle>:<label>`.
    Update {
        /// A handle, or one copy as its handle and its Interface's label or key.
        target: Option<String>,
        /// The publish sequence to move the one named copy to, in place of the head.
        #[arg(long, value_name = "SEQUENCE")]
        at: Option<NonZeroU64>,
        /// Print the address to open for a sign-in, without opening a browser.
        #[arg(long)]
        no_browser: bool,
        #[command(flatten)]
        output: Output,
    },
}

impl InterfaceAction {
    pub fn split(&self) -> (interface::Action, bool) {
        match self {
            Self::Add {
                interface,
                at,
                remote,
                no_browser,
                output,
            } => (
                interface::Action::Add {
                    interface: interface.clone(),
                    at: *at,
                    remote: remote.clone(),
                    browser: !no_browser,
                },
                output.json(),
            ),
            Self::Update {
                target,
                at,
                no_browser,
                output,
            } => (
                interface::Action::Update {
                    target: target.clone(),
                    at: *at,
                    browser: !no_browser,
                },
                output.json(),
            ),
        }
    }
}
