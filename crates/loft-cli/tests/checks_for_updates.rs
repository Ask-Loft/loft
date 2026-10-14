//! Checking for updates: every command whose Feature in this repository's own `spec/` lists the
//! tag `checks-for-updates` asks GitHub once, every other command asks nothing, and every
//! command that sends telemetry also checks for updates.

use crate::commands::{COMMANDS, Fixture, fixture};
use crate::common::EPOCH;
use crate::common::telemetry::asking;
use crate::conformance::{tagged, tagged_commands};
use software_plant::releases::Release;
use software_plant::{Clock, GitScenario, ReleaseFeed};

const TAG: &str = "checks-for-updates";

/// The number of requests a run of `args` made of the release feed, in a clean repository.
fn asked(kind: Fixture, args: &[&str]) -> usize {
    let scratch = fixture(kind);
    let feed = ReleaseFeed::listing(vec![Release::published("v99.0.0")]);
    let git = GitScenario::clean_on("main");
    let mut command = scratch.command(Some(&git), &Clock::fixed(EPOCH));
    asking(&mut command, &feed.url())
        .args(args)
        .output()
        .unwrap();
    feed.requests().len()
}

#[test]
fn a_command_asks_github_once_exactly_when_its_feature_lists_checks_for_updates() {
    let tagged = tagged_commands(TAG);
    assert!(!tagged.is_empty(), "no Feature lists `{TAG}`");
    for (slug, args, kind) in COMMANDS {
        let expected = usize::from(tagged.iter().any(|(name, ..)| *name == slug));
        assert_eq!(asked(kind, args), expected, "{slug}");
    }
}

#[test]
fn each_feature_that_lists_sends_telemetry_lists_checks_for_updates() {
    let checks = tagged(TAG);
    for slug in tagged("sends-telemetry") {
        assert!(checks.contains(&slug), "{slug}");
    }
}
