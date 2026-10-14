//! Sending a command event: every command whose Feature in this repository's own `spec/` lists
//! the tag `sends-telemetry` sends exactly one event that follows the event schema and names the
//! command, every other command sends none, and the schema names exactly the tagged commands.

use crate::commands::{COMMANDS, Fixture, fixture};
use crate::common::telemetry::{EVENT_SCHEMA, asking, command, seen};
use crate::conformance::tagged_commands;
use crate::schema;
use serde_json::Value;
use software_plant::{Collector, GitScenario};

const TAG: &str = "sends-telemetry";

/// The name an event gives the command `args` runs: its subcommand, with the second word of a
/// subcommand that has one, such as `import v1`.
fn event_name(args: &[&str]) -> String {
    let words = if ["import", "skill"].contains(&args[0]) {
        2
    } else {
        1
    };
    args[..words].join(" ")
}

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
fn a_command_sends_one_valid_event_exactly_when_its_feature_lists_sends_telemetry() {
    let tagged = tagged_commands(TAG);
    assert!(!tagged.is_empty(), "no Feature lists `{TAG}`");
    for (slug, args, kind) in COMMANDS {
        let requests = requests(kind, args);
        if tagged.iter().any(|(name, ..)| *name == slug) {
            assert_eq!(requests.len(), 1, "{slug}");
            let event = schema::conforming(&requests[0].body, EVENT_SCHEMA);
            assert_eq!(event["command"], event_name(args).as_str(), "{slug}");
        } else {
            assert!(requests.is_empty(), "{slug} sends without `{TAG}`");
        }
    }
}

#[test]
fn the_event_schema_names_exactly_the_commands_that_list_sends_telemetry() {
    let schema: Value = serde_json::from_str(EVENT_SCHEMA).unwrap();
    let mut listed: Vec<&str> = schema["properties"]["command"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|name| name.as_str().unwrap())
        .collect();
    listed.sort_unstable();
    let mut tagged: Vec<String> = tagged_commands(TAG)
        .into_iter()
        .map(|(_, args, _)| event_name(args))
        .collect();
    tagged.sort();
    assert_eq!(listed, tagged);
}

#[test]
fn the_switch_subcommands_open_no_connection() {
    let scratch = fixture(Fixture::Empty);
    let collector = Collector::accepting();
    let feed = software_plant::ReleaseFeed::listing(vec![]);
    for args in [
        ["telemetry", "off"],
        ["telemetry", "on"],
        ["telemetry", "status"],
        ["update-check", "off"],
        ["update-check", "on"],
        ["update-check", "status"],
    ] {
        let mut command = command(&scratch, None, &collector);
        let output = asking(&mut command, &feed.url())
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0), "{args:?}");
    }
    assert!(collector.requests().is_empty());
    assert!(feed.requests().is_empty());
}
