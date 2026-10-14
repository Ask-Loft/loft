//! Needing a repository: the rule every command tagged `needs-repository` follows outside a git
//! repository, run against each command whose Feature in this repository's own `spec/` lists the
//! tag; and the converse, that every command that refuses outside a repository lists it.

use std::fs;
use std::process::Output;

use crate::commands::{COMMANDS, Fixture};
use crate::common::{EPOCH, Scratch, stderr};
use crate::conformance::tagged;
use crate::snapshot::snapshot;
use software_plant::{Clock, GitScenario};

const TAG: &str = "needs-repository";

/// The arguments that run a command where it needs a repository: `loft merge-driver` needs one
/// only to install the driver.
fn args_of<'a>(slug: &str, args: &'a [&'a str]) -> &'a [&'a str] {
    if slug == "loft-merge-driver" {
        &["merge-driver", "--install"]
    } else {
        args
    }
}

/// A scratch directory for a command: an empty one, or the Reading List.
fn fixture(fixture: Fixture) -> Scratch {
    let scratch = Scratch::with_reading_list();
    if fixture == Fixture::Empty {
        fs::remove_dir_all(scratch.0.join("spec")).unwrap();
    }
    scratch
}

/// Runs `loft` where git answers that the directory is outside any repository.
fn outside(scratch: &Scratch, args: &[&str]) -> Output {
    let git = GitScenario::outside_repository();
    scratch.loft_with(args, Some(&git), &Clock::fixed(EPOCH))
}

/// Whether a run refused for want of a git repository.
fn refused(output: &Output) -> bool {
    output.status.code() == Some(1) && stderr(output).contains("git repository")
}

#[test]
fn each_tagged_command_refuses_outside_a_repository_and_writes_nothing() {
    let tagged = tagged(TAG);
    assert!(!tagged.is_empty(), "no Feature lists `{TAG}`");
    for (slug, args, kind) in COMMANDS
        .into_iter()
        .filter(|(slug, ..)| tagged.contains(&(*slug).to_owned()))
    {
        let scratch = fixture(kind);
        let before = snapshot(&scratch.0);
        let output = outside(&scratch, args_of(slug, args));
        assert!(refused(&output), "{slug}: {}", stderr(&output));
        assert!(snapshot(&scratch.0) == before, "{slug}: a file changed");
    }
}

#[test]
fn each_command_that_refuses_outside_a_repository_lists_needs_repository() {
    let tagged = tagged(TAG);
    for (slug, args, kind) in COMMANDS {
        let output = outside(&fixture(kind), args_of(slug, args));
        assert_eq!(
            refused(&output),
            tagged.iter().any(|tag| tag == slug),
            "{slug}: refuses outside a repository exactly when its Feature lists `{TAG}`\n{}",
            stderr(&output)
        );
    }
}
