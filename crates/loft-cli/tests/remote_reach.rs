//! The shared rules of reaching a remote that a connected command holds to whatever it does: the
//! address is used as it is given and a redirect is never followed, the command event holds
//! neither a token nor the address, and the switches of telemetry and the update check leave
//! the remote reachable. Each runs under the remote simulator with the collector or the
//! release feed beside it.

use serde_json::json;
use software_plant::http::{Answer, Request, Responder, Response, Server};
use software_plant::remote::{RemoteScenario, TOKEN};
use software_plant::{BrowserScenario, Clock, Collector, ReleaseFeed};

use crate::common::EPOCH;
use crate::common::connected::{Connected, connected};
use crate::common::signed_in::{self, granted, keep, sign_ins};
use crate::common::telemetry::{seen, sending};
use crate::remote_sign_in_refusals::login;

#[test]
fn loft_reaches_the_address_as_it_is_given_changing_no_part_of_it() {
    // A lowercase escape, which a normaliser would put in upper case.
    let (remote, _scratch, output, _) = login(RemoteScenario::accepting("Doc%2dOne"));
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(remote.requests()[0].path, "/documents/Doc%2dOne/head");
}

/// Redirects every request to the address it holds.
struct Redirecting(String);

impl Responder for Redirecting {
    fn answer(&mut self, _: &Request) -> Answer {
        Response::redirect(&self.0).into()
    }
}

#[test]
fn a_redirect_from_the_remote_is_not_followed() {
    let run = connected(1, 1, RemoteScenario::accepting);
    let redirecting = Server::start(Redirecting(format!("{}/head", run.remote.address())));
    let address = format!("{}/documents/{}", redirecting.url(), run.history.key);
    run.change();
    let output = run.publish_by(|command| {
        command
            .env("LOFT_REMOTE_ENDPOINT", &address)
            .env("LOFT_REMOTE_TOKEN", TOKEN);
    });
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(Connected::message(&output).contains("302"));
    assert!(
        run.remote.requests().is_empty(),
        "the redirect is not followed"
    );
}

#[test]
fn the_command_event_of_a_connected_publish_holds_no_token_and_not_the_address() {
    let run = connected(1, 1, RemoteScenario::accepting);
    let kept = granted(&run.remote, 60);
    keep(&run.scratch, &json!({ run.remote.address(): kept }));
    seen(&run.scratch);
    let collector = Collector::accepting();
    run.change();
    let output = run.publish_by(|command| {
        sending(command, &collector.url());
    });
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(run.remote.refreshes(), 1, "the sign-in was refreshed");
    let now = &sign_ins(&run.scratch)[run.remote.address()];
    let events = collector.events();
    assert_eq!(events.len(), 1);
    let event = String::from_utf8_lossy(&events[0]);
    for secret in [
        kept["accessToken"].as_str().unwrap(),
        kept["refreshToken"].as_str().unwrap(),
        now["accessToken"].as_str().unwrap(),
        now["refreshToken"].as_str().unwrap(),
        &run.remote.address(),
        run.remote.origin(),
    ] {
        assert!(!event.contains(secret), "{secret} in {event}");
    }
}

#[test]
fn a_connected_publish_with_telemetry_and_the_update_check_off_reaches_the_remote_alone() {
    let run = connected(1, 1, RemoteScenario::accepting);
    seen(&run.scratch);
    let collector = Collector::accepting();
    let feed = ReleaseFeed::listing(vec![]);
    run.change();
    let output = run.publish_by(|command| {
        command
            .env("LOFT_TELEMETRY", "off")
            .env("LOFT_TELEMETRY_ENDPOINT", collector.url())
            .env("LOFT_UPDATE_CHECK", "off")
            .env("LOFT_UPDATE_ENDPOINT", feed.url());
    });
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(
        run.calls().iter().any(|call| call.ends_with("/publish")),
        "{:?}",
        run.calls()
    );
    assert!(collector.requests().is_empty(), "no command event is sent");
    assert!(feed.requests().is_empty(), "GitHub is asked nothing");
}

#[test]
fn a_wait_counts_toward_the_time_limit_of_neither_call() {
    // The wait runs on the real clock and is longer than either call may take.
    let run = connected(1, 1, |key| {
        RemoteScenario::failing(key, 503, Some("service_unavailable")).retry_after("2")
    });
    run.change();
    let (mut command, _) = signed_in::command_at(
        &run.scratch,
        BrowserScenario::Absent,
        Some(&run.git),
        &Clock::fixed(EPOCH),
    );
    let output = command
        .args(["publish", "--format", "json"])
        .env("LOFT_REMOTE_TIMEOUT", "1")
        .env("LOFT_REMOTE_WAIT", "3")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert_eq!(
        run.calls().len(),
        2,
        "the call is made again after the wait"
    );
    let message = Connected::message(&output);
    assert!(
        message.contains("503") && !message.contains("cannot be reached"),
        "{message}"
    );
}
