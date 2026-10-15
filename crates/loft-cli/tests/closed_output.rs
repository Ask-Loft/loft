//! A command whose standard output is closed before it finishes, as when a reader such as
//! `head -1` stops reading: it exits with the code it exits with when its output is open, and
//! writes nothing to standard error about the closed output. Each command is run twice over a
//! fresh fixture, once with its output open and once into a pipe whose reading end is already
//! closed, so that its first write fails.

use std::io;
use std::process::{Output, Stdio};

use crate::commands::{COMMANDS, Fixture, command, fixture};
use crate::common::stderr;

/// One run of `loft`: a name for its failure messages, its arguments, its fixture, and whether
/// it fails on its findings.
struct Case {
    name: String,
    args: Vec<&'static str>,
    fixture: Fixture,
    fails: bool,
}

impl Case {
    fn new(name: &str, args: &[&'static str], fixture: Fixture) -> Self {
        Self {
            name: name.to_owned(),
            args: args.to_vec(),
            fixture,
            fails: false,
        }
    }

    /// The case, run where it fails on its findings.
    fn failing(name: &str, args: &[&'static str]) -> Self {
        Self {
            fails: true,
            ..Self::new(name, args, Fixture::Clean)
        }
    }

    /// Runs the case over a fresh fixture, with standard output closed when `closed`.
    fn run(&self, closed: bool) -> Output {
        let scratch = fixture(self.fixture);
        let mut command = command(&scratch, self.fixture);
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
        .map(|(slug, args, fixture, _)| {
            let mut args = args.to_vec();
            args.extend(["--format", "json"]);
            Case::new(slug, &args, *fixture)
        })
        .collect();
    cases.push(Case::new("reconcile", &["reconcile"], Fixture::Clean));
    cases.push(Case::failing(
        "reconcile failing on info",
        &["reconcile", "--fail-on", "info"],
    ));
    cases.push(Case::failing(
        "check failing on info",
        &["check", "--fail-on", "info"],
    ));
    cases
}

#[test]
fn every_command_with_its_output_closed_exits_as_when_open_and_writes_nothing_about_it() {
    for case in cases() {
        let open = case.run(false);
        let closed = case.run(true);
        let text = stderr(&closed);
        assert_eq!(
            closed.status.code(),
            open.status.code(),
            "{}: the exit code with the output closed\n{text}",
            case.name
        );
        if case.fails {
            assert_eq!(open.status.code(), Some(1), "{}", case.name);
        }
        for mention in ["panicked", "Broken pipe"] {
            assert!(!text.contains(mention), "{}: {mention}\n{text}", case.name);
        }
        assert_eq!(text, stderr(&open), "{}: standard error", case.name);
    }
}
