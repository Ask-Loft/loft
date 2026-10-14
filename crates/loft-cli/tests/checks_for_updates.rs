//! Checking for updates: the rule every command tagged `checks-for-updates` follows, run against
//! each command whose Feature in this repository's own `spec/` lists the tag; the converse, that
//! every command that asks GitHub lists it; and that every command that sends telemetry lists it
//! too.

use std::fs;

use crate::commands::{COMMANDS, Fixture};
use crate::common::{EPOCH, Scratch};
use crate::conformance::tagged;
use software_plant::releases::Release;
use software_plant::{Clock, GitScenario, ReleaseFeed};

const TAG: &str = "checks-for-updates";

/// The number of requests a run of `args` made of the release feed, in a clean repository.
fn asked(kind: Fixture, args: &[&str]) -> usize {
    let scratch = Scratch::with_reading_list();
    if kind == Fixture::Empty {
        fs::remove_dir_all(scratch.0.join("spec")).unwrap();
    }
    let feed = ReleaseFeed::listing(vec![Release::published("v99.0.0")]);
    let git = GitScenario::clean_on("main");
    scratch
        .command(Some(&git), &Clock::fixed(EPOCH))
        .env_remove("LOFT_UPDATE_CHECK")
        .env("LOFT_UPDATE_ENDPOINT", feed.url())
        .args(args)
        .output()
        .unwrap();
    feed.requests().len()
}

#[test]
fn each_tagged_command_asks_github_once() {
    let tagged = tagged(TAG);
    assert!(!tagged.is_empty(), "no Feature lists `{TAG}`");
    for (slug, args, kind) in COMMANDS
        .into_iter()
        .filter(|(slug, ..)| tagged.contains(&(*slug).to_owned()))
    {
        assert_eq!(asked(kind, args), 1, "{slug}");
    }
}

#[test]
fn each_command_that_asks_github_lists_checks_for_updates() {
    let tagged = tagged(TAG);
    for (slug, args, kind) in COMMANDS {
        let asks = asked(kind, args) > 0;
        assert_eq!(
            asks,
            tagged.iter().any(|tag| tag == slug),
            "{slug}: asks {asks}"
        );
    }
}

#[test]
fn each_feature_that_lists_sends_telemetry_lists_checks_for_updates() {
    let checks = tagged(TAG);
    for slug in tagged("sends-telemetry") {
        assert!(checks.contains(&slug), "{slug}");
    }
}
