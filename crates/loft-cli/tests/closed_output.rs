//! A command whose standard output is closed before it finishes, as when a reader such as
//! `head -1` stops reading: it exits with the code it exits with when its output is open, and
//! writes nothing to standard error about the closed output. Each command is run twice over a
//! fresh fixture, once with its output open and once into a pipe whose reading end is already
//! closed, so that its first write fails.

use std::io;
use std::process::{Output, Stdio};

use crate::commands::{COMMANDS, Fixture};
use crate::common::{EPOCH, Scratch, stderr};
use crate::writes_tree::fixture;
use software_plant::{Clock, GitScenario};

/// One run of `loft`: a name for its failure messages, its arguments and its fixture.
struct Case {
    name: String,
    args: Vec<&'static str>,
    fixture: Fixture,
}

impl Case {
    fn new(name: &str, args: &[&'static str], fixture: Fixture) -> Self {
        Self {
            name: name.to_owned(),
            args: args.to_vec(),
            fixture,
        }
    }

    /// Runs the case over a fresh fixture: in a clean repository on `main` for the clean
    /// fixture, with no `git` otherwise; with standard output closed when `closed`.
    fn run(&self, closed: bool) -> Output {
        let scratch: Scratch = fixture(self.fixture);
        let git = (self.fixture == Fixture::Clean).then(|| GitScenario::clean_on("main"));
        let mut command = scratch.command(git.as_ref(), &Clock::fixed(EPOCH));
        command.args(&self.args).stderr(Stdio::piped());
        if closed {
            let (reader, writer) = io::pipe().unwrap();
            drop(reader);
            command.stdout(writer);
        }
        command.output().unwrap()
    }
}

/// Every command with `--format json`, which prints its document on standard output; and the
/// plain text of `loft reconcile` and `loft check`, which print a line for each finding, passing
/// and failing.
fn cases() -> Vec<Case> {
    let mut cases: Vec<Case> = COMMANDS
        .iter()
        .map(|(slug, args, fixture)| {
            let mut args = args.to_vec();
            args.extend(["--format", "json"]);
            Case::new(slug, &args, *fixture)
        })
        .collect();
    cases.push(Case::new("reconcile", &["reconcile"], Fixture::Clean));
    cases.extend(failing());
    cases
}

/// Commands that print a line for each finding and then fail on them.
fn failing() -> [Case; 2] {
    [
        Case::new(
            "reconcile failing on info",
            &["reconcile", "--fail-on", "info"],
            Fixture::Clean,
        ),
        Case::new(
            "check failing on info",
            &["check", "--fail-on", "info"],
            Fixture::Clean,
        ),
    ]
}

#[test]
fn every_command_with_its_output_closed_exits_with_the_code_it_exits_with_when_open() {
    for case in cases() {
        let open = case.run(false);
        let closed = case.run(true);
        assert_eq!(
            closed.status.code(),
            open.status.code(),
            "{}: the exit code with the output closed\n{}",
            case.name,
            stderr(&closed)
        );
    }
}

#[test]
fn every_command_with_its_output_closed_writes_nothing_about_it_to_standard_error() {
    for case in cases() {
        let open = case.run(false);
        let closed = case.run(true);
        let text = stderr(&closed);
        for mention in ["panicked", "Broken pipe"] {
            assert!(!text.contains(mention), "{}: {mention}\n{text}", case.name);
        }
        assert_eq!(text, stderr(&open), "{}: standard error", case.name);
    }
}

#[test]
fn a_command_that_fails_on_its_findings_still_exits_1_with_its_output_closed() {
    for case in failing() {
        assert_eq!(case.run(false).status.code(), Some(1), "{}", case.name);
        assert_eq!(case.run(true).status.code(), Some(1), "{}", case.name);
    }
}
