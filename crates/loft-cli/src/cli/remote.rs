//! The arguments of `loft remote`.

use clap::Subcommand;

use super::Output;
use crate::remote_command;

#[derive(Subcommand)]
pub enum RemoteAction {
    /// Connect the document to its remote at the address, signing in where needed, and name it
    /// in the manifest.
    Set {
        #[arg(help = concat!(
            "The document's own address on your Loft server, not the server's: ",
            crate::remote::document_address_form!(),
            ", where <documentKey> is the `documentKey` of spec/loftspec.yaml"
        ))]
        address: String,
        /// Print the address to open, without opening a browser.
        #[arg(long)]
        no_browser: bool,
        #[command(flatten)]
        output: Output,
    },
    /// Sign in again to the remote that the manifest names, once a sign-in has lapsed or in a
    /// fresh clone.
    Login {
        /// Print the address to open, without opening a browser.
        #[arg(long)]
        no_browser: bool,
        #[command(flatten)]
        output: Output,
    },
    /// Disconnect the document from its remote and forget the sign-in to it.
    Remove {
        #[command(flatten)]
        output: Output,
    },
}

impl RemoteAction {
    pub fn split(&self) -> (remote_command::Action, bool) {
        match self {
            Self::Set {
                address,
                no_browser,
                output,
            } => (
                remote_command::Action::Set {
                    address: address.clone(),
                    browser: !no_browser,
                },
                output.json(),
            ),
            Self::Login { no_browser, output } => (
                remote_command::Action::Login {
                    browser: !no_browser,
                },
                output.json(),
            ),
            Self::Remove { output } => (remote_command::Action::Remove, output.json()),
        }
    }
}
