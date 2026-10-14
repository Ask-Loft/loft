//! Checking for updates: GitHub asked at the end of a run at most once a day, the answer kept,
//! and a newer release told of on a later run, at most once a day, with the way to install it
//! that matches how `loft` was installed.

use std::fs;
use std::process::Output;

use crate::common::telemetry::asking;
use crate::common::{EPOCH, Scratch, stderr, stdout};
use serde_json::json;
use software_plant::http::{Request, Responder, Response, Server};
use software_plant::releases::{LATEST, Release};
use software_plant::{Clock, ReleaseFeed};

const DAY: u64 = 86_400;

/// The settings file the update check keeps its answer in.
const UPDATE_CHECK_FILE: &str = "update-check.json";

/// `loft check` in the Reading List at `at`, with the update check on and asking `feed`.
fn check_at(scratch: &Scratch, address: &str, at: u64) -> Output {
    let mut command = scratch.command(None, &Clock::fixed(at));
    asking(&mut command, address);
    command.arg("check").output().unwrap()
}

/// A feed whose latest release is newer than any `loft` under test.
fn newer() -> ReleaseFeed {
    ReleaseFeed::listing(vec![Release::published("v99.0.0")])
}

#[test]
fn the_latest_release_is_asked_after_the_output_and_kept_with_its_time() {
    let scratch = Scratch::with_reading_list();
    let feed = newer();
    let output = check_at(&scratch, &feed.url(), EPOCH);
    assert_eq!(feed.requests().len(), 1);
    assert_eq!(feed.requests()[0].path, LATEST);
    assert_eq!(
        scratch.setting(UPDATE_CHECK_FILE)["latest"]["version"],
        json!("99.0.0")
    );
    assert_eq!(
        scratch.setting(UPDATE_CHECK_FILE)["receivedAt"],
        json!(EPOCH)
    );
    assert_eq!(stderr(&output), "");
}

#[test]
fn a_run_within_a_day_of_the_last_answer_asks_nothing() {
    let scratch = Scratch::with_reading_list();
    let feed = newer();
    check_at(&scratch, &feed.url(), EPOCH);
    check_at(&scratch, &feed.url(), EPOCH + DAY - 1);
    assert_eq!(feed.requests().len(), 1);
    check_at(&scratch, &feed.url(), EPOCH + DAY);
    assert_eq!(feed.requests().len(), 2);
}

#[test]
fn a_newer_release_is_told_of_on_a_later_run_once_a_day() {
    let scratch = Scratch::with_reading_list();
    let feed = newer();
    let first = check_at(&scratch, &feed.url(), EPOCH);
    assert_eq!(stderr(&first), "");
    let second = check_at(&scratch, &feed.url(), EPOCH + 60);
    let notice = stderr(&second);
    assert!(notice.contains("99.0.0"), "{notice}");
    assert!(notice.contains(env!("CARGO_PKG_VERSION")), "{notice}");
    assert_eq!(stdout(&second), stdout(&first));
    assert_eq!(stderr(&check_at(&scratch, &feed.url(), EPOCH + 120)), "");
    let next_day = check_at(&scratch, &feed.url(), EPOCH + 60 + DAY);
    assert!(
        stderr(&next_day).contains("99.0.0"),
        "{}",
        stderr(&next_day)
    );
}

#[test]
fn the_notice_is_printed_in_every_mode() {
    let modes = [
        ("CI", "true"),
        ("CLAUDECODE", "1"),
        ("LOFT_TELEMETRY", "internal"),
    ];
    for (name, value) in modes {
        let scratch = Scratch::with_reading_list();
        crate::common::telemetry::seen(&scratch);
        let feed = newer();
        check_at(&scratch, &feed.url(), EPOCH);
        let mut command = scratch.command(None, &Clock::fixed(EPOCH + 60));
        asking(&mut command, &feed.url());
        let output = command.env(name, value).arg("check").output().unwrap();
        assert!(
            stderr(&output).contains("99.0.0"),
            "{name}: {}",
            stderr(&output)
        );
    }
}

#[cfg(unix)]
#[test]
fn the_notice_is_printed_on_a_terminal_too() {
    let scratch = Scratch::with_reading_list();
    let feed = newer();
    check_at(&scratch, &feed.url(), EPOCH);
    let mut command = scratch.check_on_terminal(&Clock::fixed(EPOCH + 60));
    let output = asking(&mut command, &feed.url()).output().unwrap();
    let shown = String::from_utf8_lossy(&output.stdout);
    assert!(shown.contains("99.0.0"), "{shown}");
}

#[test]
fn the_running_version_or_an_older_one_is_told_of_by_no_notice() {
    for tag in [
        format!("v{}", env!("CARGO_PKG_VERSION")),
        "v0.0.1".to_owned(),
    ] {
        let scratch = Scratch::with_reading_list();
        let feed = ReleaseFeed::listing(vec![Release::published(&tag)]);
        check_at(&scratch, &feed.url(), EPOCH);
        assert_eq!(
            stderr(&check_at(&scratch, &feed.url(), EPOCH + 60)),
            "",
            "{tag}"
        );
    }
}

#[test]
fn a_draft_or_a_prerelease_is_not_taken_as_the_latest() {
    for release in [Release::draft("v99.0.0"), Release::prerelease("v99.0.0")] {
        let scratch = Scratch::with_reading_list();
        let feed = ReleaseFeed::answering(release);
        check_at(&scratch, &feed.url(), EPOCH);
        assert!(
            scratch.setting(UPDATE_CHECK_FILE).get("latest").is_none(),
            "{}",
            scratch.setting(UPDATE_CHECK_FILE)
        );
        assert_eq!(stderr(&check_at(&scratch, &feed.url(), EPOCH + 60)), "");
    }
}

#[test]
fn a_tag_that_is_not_a_version_is_told_of_by_no_notice() {
    for tag in ["nightly", "99.0.0", "v99.0"] {
        let scratch = Scratch::with_reading_list();
        let feed = ReleaseFeed::listing(vec![Release::published(tag)]);
        assert_eq!(stderr(&check_at(&scratch, &feed.url(), EPOCH)), "", "{tag}");
        assert_eq!(
            stderr(&check_at(&scratch, &feed.url(), EPOCH + 60)),
            "",
            "{tag}"
        );
    }
}

#[test]
fn the_notice_names_the_way_to_update_that_matches_the_install() {
    let cases = [
        (Some("npm"), "npm install @ask-loft/loft@latest"),
        (Some("cargo"), "cargo install --locked loft-cli"),
        (
            Some("release-binary"),
            "https://github.com/Ask-Loft/loft/releases/tag/v99.0.0",
        ),
        (
            None,
            "https://github.com/Ask-Loft/loft/releases/tag/v99.0.0",
        ),
    ];
    for (way, line) in cases {
        let scratch = Scratch::with_reading_list();
        if let Some(way) = way {
            scratch.record_install(way);
        }
        let feed = newer();
        check_at(&scratch, &feed.url(), EPOCH);
        let notice = stderr(&check_at(&scratch, &feed.url(), EPOCH + 60));
        assert!(notice.contains(line), "{way:?}: {notice}");
    }
}

#[test]
fn an_npm_install_whose_script_never_ran_still_names_npm() {
    let scratch = Scratch::with_reading_list();
    let feed = newer();
    let run = |at: u64| {
        let mut command = scratch.npm_installed(&Clock::fixed(at));
        asking(&mut command, &feed.url());
        command.arg("check").output().unwrap()
    };
    run(EPOCH);
    let notice = stderr(&run(EPOCH + 60));
    assert!(
        notice.contains("npm install @ask-loft/loft@latest"),
        "{notice}"
    );
}

#[test]
fn a_feed_that_refuses_fails_rate_limits_or_stays_silent_changes_nothing() {
    let scratch = Scratch::with_reading_list();
    let feed = newer();
    let answered = check_at(&scratch, &feed.url(), EPOCH);
    let before = scratch.setting(UPDATE_CHECK_FILE);
    let failing = ReleaseFeed::failing(500);
    let limited = ReleaseFeed::rate_limited();
    let silent = ReleaseFeed::silent();
    for address in [
        ReleaseFeed::refusing(),
        failing.url(),
        limited.url(),
        silent.url(),
    ] {
        // A day later the kept answer is stale, so the run asks again.
        let mut command = scratch.command(None, &Clock::fixed(EPOCH + DAY));
        asking(&mut command, &address);
        let output = command.args(["check"]).output().unwrap();
        assert_eq!(output.status.code(), answered.status.code(), "{address}");
        assert_eq!(stdout(&output), stdout(&answered), "{address}");
        let kept_after = scratch.setting(UPDATE_CHECK_FILE);
        assert_eq!(kept_after["latest"], before["latest"], "{address}");
        assert_eq!(kept_after["receivedAt"], before["receivedAt"], "{address}");
    }
}

#[test]
fn a_failed_request_writes_nothing_to_standard_error() {
    let failing = ReleaseFeed::failing(500);
    let limited = ReleaseFeed::rate_limited();
    let silent = ReleaseFeed::silent();
    for address in [
        ReleaseFeed::refusing(),
        failing.url(),
        limited.url(),
        silent.url(),
    ] {
        let scratch = Scratch::with_reading_list();
        let output = check_at(&scratch, &address, EPOCH);
        assert_eq!(stderr(&output), "", "{address}");
        assert_eq!(
            stderr(&check_at(&scratch, &address, EPOCH + DAY)),
            "",
            "{address}"
        );
    }
}

#[test]
fn a_run_that_cannot_keep_the_answer_asks_github_nothing() {
    let scratch = Scratch::with_reading_list();
    let blocked = scratch.0.join("not-a-directory");
    fs::write(&blocked, "").unwrap();
    let feed = newer();
    let mut command = scratch.command(None, &Clock::fixed(EPOCH));
    asking(&mut command, &feed.url());
    let output = command
        .env("HOME", &blocked)
        .env("APPDATA", &blocked)
        .arg("check")
        .output()
        .unwrap();
    assert!(feed.requests().is_empty());
    assert_eq!(stderr(&output), "");
}

#[cfg(unix)]
#[test]
fn a_run_that_cannot_keep_the_answer_prints_no_update_notice() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = Scratch::with_reading_list();
    let feed = newer();
    check_at(&scratch, &feed.url(), EPOCH);
    let config = scratch.config();
    fs::set_permissions(&config, fs::Permissions::from_mode(0o555)).unwrap();
    let output = check_at(&scratch, &feed.url(), EPOCH + DAY + 60);
    fs::set_permissions(&config, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(stderr(&output), "");
    assert_eq!(feed.requests().len(), 1);
}

/// Answers every request with a redirect to `to`.
struct Redirecting(String);

impl Responder for Redirecting {
    fn answer(&mut self, _: &Request) -> Option<Response> {
        let mut response = Response::empty(302);
        response
            .headers
            .push(("Location".to_owned(), self.0.clone()));
        Some(response)
    }
}

#[test]
fn a_redirect_is_not_followed() {
    let scratch = Scratch::with_reading_list();
    let target = newer();
    let redirecting = Server::start(Redirecting(format!("{}{LATEST}", target.url())));
    let output = check_at(&scratch, &redirecting.url(), EPOCH);
    assert_eq!(redirecting.requests().len(), 1);
    assert!(target.requests().is_empty());
    assert!(
        scratch.setting(UPDATE_CHECK_FILE).get("latest").is_none(),
        "{}",
        scratch.setting(UPDATE_CHECK_FILE)
    );
    assert_eq!(stderr(&output), "");
}

#[test]
fn while_the_update_check_is_off_nothing_is_asked_or_told() {
    for variable in [Some("off"), None] {
        let scratch = Scratch::with_reading_list();
        let feed = newer();
        check_at(&scratch, &feed.url(), EPOCH);
        if variable.is_none() {
            let off = scratch
                .command(None, &Clock::fixed(EPOCH))
                .args(["update-check", "off"])
                .output()
                .unwrap();
            assert_eq!(off.status.code(), Some(0));
        }
        let mut command = scratch.command(None, &Clock::fixed(EPOCH + DAY + 60));
        asking(&mut command, &feed.url());
        if let Some(value) = variable {
            command.env("LOFT_UPDATE_CHECK", value);
        }
        let output = command.arg("check").output().unwrap();
        assert_eq!(stderr(&output), "", "{variable:?}");
        assert_eq!(feed.requests().len(), 1, "{variable:?}");
    }
}

#[test]
fn the_request_carries_nothing_but_the_address_and_the_name_loft() {
    let scratch = Scratch::with_reading_list();
    let feed = newer();
    check_at(&scratch, &feed.url(), EPOCH);
    let request = &feed.requests()[0];
    assert_eq!(request.method, "GET");
    assert_eq!(request.path, LATEST);
    assert_eq!(request.body, Vec::<u8>::new());
    assert_eq!(request.header("user-agent"), Some("loft"));
    let names: Vec<&str> = request
        .headers
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    for name in names {
        assert!(
            [
                "host",
                "user-agent",
                "accept",
                "accept-encoding",
                "connection"
            ]
            .contains(&name),
            "{name}"
        );
    }
}

#[test]
fn switching_telemetry_off_leaves_the_update_check_asking() {
    let scratch = Scratch::with_reading_list();
    let feed = newer();
    let mut command = scratch.command(None, &Clock::fixed(EPOCH));
    asking(&mut command, &feed.url());
    command
        .env("LOFT_TELEMETRY", "off")
        .arg("check")
        .output()
        .unwrap();
    assert_eq!(feed.requests().len(), 1);
}
