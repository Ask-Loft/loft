//! Sending a command event: the rule every command tagged `sends-telemetry` follows, run
//! against each command whose Feature in this repository's own `spec/` lists the tag; and the
//! converse, that every command that sends an event lists it.

use crate::commands::{COMMANDS, Fixture, fixture};
use crate::common::telemetry::{EVENT_SCHEMA, command, seen};
use crate::conformance::tagged;
use crate::schema;
use software_plant::{Collector, GitScenario};

const TAG: &str = "sends-telemetry";

/// The requests a run of `args` made of an accepting collector, in a clean repository.
fn requests(kind: Fixture, args: &[&str]) -> Vec<software_plant::http::Request> {
    let scratch = fixture(kind);
    seen(&scratch);
    let collector = Collector::accepting();
    let git = GitScenario::clean_on("main");
    command(&scratch, Some(&git), &collector)
        .args(args)
        .output()
        .unwrap();
    collector.requests()
}

#[test]
fn each_tagged_command_sends_exactly_one_valid_event() {
    let tagged = tagged(TAG);
    assert!(!tagged.is_empty(), "no Feature lists `{TAG}`");
    for (slug, args, kind) in COMMANDS
        .into_iter()
        .filter(|(slug, ..)| tagged.contains(&(*slug).to_owned()))
    {
        let requests = requests(kind, args);
        assert_eq!(requests.len(), 1, "{slug}");
        schema::conforming(&requests[0].body, EVENT_SCHEMA);
    }
}

#[test]
fn each_command_that_sends_an_event_lists_sends_telemetry() {
    let tagged = tagged(TAG);
    for (slug, args, kind) in COMMANDS {
        let sends = !requests(kind, args).is_empty();
        assert_eq!(
            sends,
            tagged.iter().any(|tag| tag == slug),
            "{slug}: sends {sends}"
        );
    }
}
