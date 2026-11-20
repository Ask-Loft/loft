//! The help of `loft remote set`'s address, which gives the form of the document's address on a
//! Loft server with the key of the manifest in the working directory, where the arguments name
//! `remote` and that manifest reads; every other command reads no manifest for its help.

use std::ffi::OsString;
use std::path::Path;

use clap::CommandFactory as _;
use loftspec::codec::MANIFEST;
use loftspec::yaml::{self, Value as Yaml};

use crate::cli::Cli;
use crate::remote;

/// The command line that parses `args`: `loft`'s own, with the help of `loft remote set`'s
/// address naming the document of the manifest in `root` where `args` name `remote`.
pub fn command(args: &[OsString], root: &Path) -> clap::Command {
    let command = Cli::command();
    let Some(key) = names_remote(args).then(|| manifest_key(root)).flatten() else {
        return command;
    };
    let help = format!(
        "The document's own address on your Loft server, not the server's: {}",
        remote::address_form_for(&key)
    );
    command.mut_subcommand("remote", |remote| {
        remote.mut_subcommand("set", |set| {
            set.mut_arg("address", |address| address.help(help))
        })
    })
}

/// Whether the arguments after the program's name name `remote`.
fn names_remote(args: &[OsString]) -> bool {
    args.iter().skip(1).any(|arg| arg == "remote")
}

/// The `documentKey` of the manifest under `root`, or none where it does not read.
fn manifest_key(root: &Path) -> Option<String> {
    let bytes = std::fs::read(root.join("spec").join(MANIFEST)).ok()?;
    let Yaml::Mapping(entries) = yaml::read(&bytes).ok()??.value else {
        return None;
    };
    let entry = entries
        .into_iter()
        .find(|entry| entry.key == "documentKey")?;
    match entry.value.value {
        Yaml::Scalar(scalar) => Some(scalar.text),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::names_remote;

    fn args(words: &[&str]) -> Vec<OsString> {
        words.iter().map(OsString::from).collect()
    }

    #[test]
    fn only_a_command_line_that_names_remote_reads_a_manifest_for_its_help() {
        assert!(names_remote(&args(&["loft", "remote", "set", "--help"])));
        assert!(!names_remote(&args(&["loft", "check", "--help"])));
        assert!(!names_remote(&args(&["remote"])));
    }
}
