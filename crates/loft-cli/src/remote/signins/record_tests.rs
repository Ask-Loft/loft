//! The record of each refresh of a sign-in over companion resources, run against the remote
//! simulator on a `loft/remotes.json` in a scratch directory of the test's own: the resource of a
//! refresh is recorded before it is sent and the record removed with its answer, and a recorded
//! refresh is repeated before any other refresh, whatever its answer.

use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use software_plant::remote::RemoteScenario;
use yardlog_store::lock::try_lock;

use super::access_token;
use super::fixture::{Scratch, TIMEOUT, listing_notes, over, refresh_token, sent};
use crate::remote::RemoteError;

/// `entry` without its record of a refresh.
fn unrecorded(mut entry: Value) -> Value {
    entry.as_object_mut().unwrap().remove("refreshing");
    entry
}

/// The error of a sign-in to `address` that has lapsed with `error`.
fn lapsed(address: &str, error: &str) -> RemoteError {
    RemoteError::Lapsed {
        address: address.to_owned(),
        error: error.to_owned(),
    }
}

#[test]
fn a_lost_answer_leaves_the_record_and_the_next_call_repeats_it_first() {
    let remote = listing_notes(RemoteScenario::accepting("doc-lost").losing_a_refresh_answer());
    let (address, notes) = (remote.address(), remote.companion("notes"));
    let entry = over(&remote, &["notes"], 3600, false);
    let scratch = Scratch::holding(&remote, &entry);
    let lost = access_token(&scratch.file, &address, Some(&notes), TIMEOUT);
    assert!(
        matches!(lost, Err(RemoteError::Unreachable { .. })),
        "{lost:?}"
    );
    let recorded = scratch.kept(&remote);
    assert_eq!(
        recorded["refreshing"],
        notes.as_str(),
        "the record names the resource"
    );
    assert_eq!(unrecorded(recorded), entry, "the tokens are as they were");
    // The remote's own token expires later than five minutes ahead, and the recorded refresh is
    // still repeated first, with the refresh token kept.
    let mine = access_token(&scratch.file, &address, None, TIMEOUT);
    assert_eq!(mine.as_deref(), Ok(entry["accessToken"].as_str().unwrap()));
    let repeated = (refresh_token(&entry), vec![notes.clone()]);
    assert_eq!(sent(&remote), [repeated.clone(), repeated]);
    assert_eq!(
        remote.refreshes(),
        1,
        "the repeat is answered as the first was"
    );
    let now = scratch.kept(&remote);
    assert!(now.get("refreshing").is_none(), "{now}");
    assert_ne!(
        now["refreshToken"], entry["refreshToken"],
        "the successor is kept"
    );
    let theirs = now["resources"][0]["accessToken"].as_str().unwrap();
    assert!(
        remote.admits_at(&notes, theirs),
        "what the repeat issued is kept"
    );
}

#[test]
fn a_refresh_is_recorded_under_the_lock_before_it_is_sent() {
    // The simulator answers the refresh only after a delay, and records it only once it has
    // answered: a record read while it has recorded none was written before its answer, which
    // a call that waits for its answer can only have done before it sent the refresh.
    let scenario = RemoteScenario::accepting("doc-recorded-first")
        .delaying_refreshes(Duration::from_millis(500));
    let remote = listing_notes(scenario);
    let (address, notes) = (remote.address(), remote.companion("notes"));
    let entry = over(&remote, &["notes"], 3600, false);
    let scratch = Scratch::holding(&remote, &entry);
    let file = &scratch.file;
    let lock = file.with_file_name("remotes.json.lock");
    let token = thread::scope(|scope| {
        let call = scope.spawn(|| access_token(file, &address, Some(&notes), TIMEOUT));
        let deadline = Instant::now() + Duration::from_secs(10);
        while scratch.kept(&remote)["refreshing"] != notes.as_str() {
            assert!(Instant::now() < deadline, "the refresh is recorded");
            thread::sleep(Duration::from_millis(5));
        }
        let answered = sent(&remote);
        let first = "recorded in loft/remotes.json before the refresh is answered";
        assert!(answered.is_empty(), "{first}: {answered:?}");
        let free = try_lock(&lock).unwrap().is_some();
        assert!(
            !free,
            "the lock beside loft/remotes.json is held while it is recorded"
        );
        call.join().unwrap()
    });
    let token = token.unwrap();
    let now = scratch.kept(&remote);
    assert!(now.get("refreshing").is_none(), "{now}");
    assert_eq!(now["resources"][0]["accessToken"], token.as_str());
}

#[test]
fn a_recorded_refresh_refused_invalid_target_keeps_the_record_and_sends_no_other_refresh() {
    // Whether the remote's own token expires within five minutes or not.
    for seconds in [60, 3600] {
        let remote = listing_notes(RemoteScenario::accepting("doc-retired"));
        let address = remote.address();
        // A resource the remote does not serve, as one it served before.
        let retired = format!("{}/retired", remote.origin());
        let mut entry = over(&remote, &["notes"], seconds, true);
        entry["refreshing"] = json!(retired);
        let scratch = Scratch::holding(&remote, &entry);
        let before = scratch.bytes();
        let refused = access_token(&scratch.file, &address, None, TIMEOUT).unwrap_err();
        let error = format!("its refresh recorded for {retired} was refused with `invalid_target`");
        assert_eq!(
            refused,
            lapsed(&address, &error),
            "{seconds}: no token is given"
        );
        let message = refused.to_string();
        for said in [retired.as_str(), "has lapsed", "loft remote login"] {
            assert!(message.contains(said), "{seconds}: {said}: {message}");
        }
        let repeated = (refresh_token(&entry), vec![retired]);
        assert_eq!(sent(&remote), [repeated], "{seconds}: no other refresh");
        assert_eq!(remote.refreshes(), 0, "{seconds}");
        assert_eq!(scratch.bytes(), before, "{seconds}: the record stays");
    }
}

#[test]
fn a_refused_refresh_removes_its_record_and_leaves_the_sign_in_as_it_was() {
    let remote = listing_notes(RemoteScenario::accepting("doc-refused").lapsing());
    let address = remote.address();
    let entry = over(&remote, &["notes"], 60, true);
    let scratch = Scratch::holding(&remote, &entry);
    let refused = access_token(&scratch.file, &address, None, TIMEOUT).unwrap_err();
    assert_eq!(refused, lapsed(&address, "invalid_grant"));
    assert!(
        refused.to_string().contains("loft remote login"),
        "{refused}"
    );
    let refreshed = (refresh_token(&entry), vec![remote.resource()]);
    assert_eq!(sent(&remote), [refreshed]);
    assert_eq!(
        scratch.kept(&remote),
        entry,
        "its record removed, the sign-in is as it was"
    );
}

#[test]
fn a_repeat_refused_with_another_error_leaves_the_file_as_it_was() {
    let remote = listing_notes(RemoteScenario::accepting("doc-repeat-lapsed").lapsing());
    let (address, notes) = (remote.address(), remote.companion("notes"));
    let mut entry = over(&remote, &["notes"], 60, true);
    entry["refreshing"] = json!(notes);
    let scratch = Scratch::holding(&remote, &entry);
    let before = scratch.bytes();
    let refused = access_token(&scratch.file, &address, None, TIMEOUT);
    assert_eq!(refused, Err(lapsed(&address, "invalid_grant")));
    let repeated = (refresh_token(&entry), vec![notes]);
    assert_eq!(sent(&remote), [repeated], "no other refresh follows");
    assert_eq!(scratch.bytes(), before, "the record stays");
}

#[test]
fn a_refresh_for_another_resource_is_sent_only_once_the_repeat_s_answer_is_kept() {
    // A refresh token presented again would revoke the sign-in.
    let remote = listing_notes(RemoteScenario::accepting("doc-repeat-first").without_grace());
    let (address, own, notes) = (
        remote.address(),
        remote.resource(),
        remote.companion("notes"),
    );
    // A record for notes, and a token for the remote's own resource that expires soon.
    let mut entry = over(&remote, &["notes"], 60, false);
    entry["refreshing"] = json!(notes);
    let scratch = Scratch::holding(&remote, &entry);
    let mine = access_token(&scratch.file, &address, None, TIMEOUT).unwrap();
    assert!(remote.admits_at(&own, &mine));
    let refreshes = sent(&remote);
    assert_eq!(refreshes.len(), 2, "{refreshes:?}");
    let repeat = (refresh_token(&entry), vec![notes.clone()]);
    assert_eq!(refreshes[0], repeat, "the repeat first");
    assert_eq!(
        refreshes[1].1,
        [own],
        "then the refresh for the remote's own resource"
    );
    assert_ne!(
        refreshes[1].0, refreshes[0].0,
        "with the successor the repeat kept"
    );
    let now = scratch.kept(&remote);
    assert!(now.get("refreshing").is_none(), "{now}");
    assert_eq!(now["accessToken"], mine.as_str());
    let theirs = now["resources"][0]["accessToken"].as_str().unwrap();
    assert!(
        remote.admits_at(&notes, theirs),
        "what the repeat issued is kept"
    );
}

#[test]
fn a_repeat_whose_answer_is_lost_again_keeps_the_record_and_gives_a_live_token() {
    // Whether the remote's own token expires within five minutes or not.
    for seconds in [60, 3600] {
        let scenario = RemoteScenario::accepting("doc-lost-again").losing_a_refresh_answer();
        let remote = listing_notes(scenario);
        let (address, notes) = (remote.address(), remote.companion("notes"));
        // A record whose refresh never reached the remote.
        let mut entry = over(&remote, &["notes"], seconds, true);
        entry["refreshing"] = json!(notes);
        let scratch = Scratch::holding(&remote, &entry);
        let before = scratch.bytes();
        let given = access_token(&scratch.file, &address, None, TIMEOUT);
        if seconds == 3600 {
            assert_eq!(given.as_deref(), Ok(entry["accessToken"].as_str().unwrap()));
        } else {
            let lost = matches!(given, Err(RemoteError::Unreachable { .. }));
            assert!(lost, "{given:?}");
        }
        let repeated = (refresh_token(&entry), vec![notes]);
        assert_eq!(sent(&remote), [repeated], "{seconds}: no refresh follows");
        let unchanged = "the record and the tokens are as they were";
        assert_eq!(scratch.bytes(), before, "{seconds}: {unchanged}");
    }
}

#[test]
fn an_answer_that_does_not_read_keeps_the_record_and_the_tokens() {
    // Every request is answered `200` with a page that is not a token answer.
    let remote = listing_notes(RemoteScenario::failing("doc-unreadable", 200, None));
    let address = remote.address();
    let entry = over(&remote, &["notes"], 60, true);
    let scratch = Scratch::holding(&remote, &entry);
    let refused = access_token(&scratch.file, &address, None, TIMEOUT);
    assert_eq!(
        refused,
        Err(lapsed(&address, "the token answer does not read"))
    );
    let now = scratch.kept(&remote);
    assert_eq!(
        now["refreshing"],
        remote.resource().as_str(),
        "the record stays"
    );
    assert_eq!(unrecorded(now), entry, "the tokens are as they were");
}

#[test]
fn a_refresh_that_cannot_reach_the_token_endpoint_leaves_the_record_and_the_tokens() {
    // The remote's port refuses every connection.
    let remote = listing_notes(RemoteScenario::refusing("doc-unreached"));
    let (address, notes) = (remote.address(), remote.companion("notes"));
    let entry = over(&remote, &["notes"], 3600, false);
    let scratch = Scratch::holding(&remote, &entry);
    let unreached = access_token(&scratch.file, &address, Some(&notes), TIMEOUT);
    let endpoint = format!("{}/auth/token", remote.origin());
    let unreachable = matches!(&unreached, Err(RemoteError::Unreachable { address, .. })
        if *address == endpoint);
    assert!(unreachable, "{unreached:?}");
    let now = scratch.kept(&remote);
    assert_eq!(now["refreshing"], notes.as_str(), "the record stays");
    assert_eq!(unrecorded(now), entry, "the tokens are as they were");
}

#[test]
fn a_repeat_that_cannot_reach_the_token_endpoint_keeps_the_record_and_gives_a_live_token() {
    // Whether the remote's own token expires later than five minutes ahead or not.
    for seconds in [3600, 60] {
        let remote = listing_notes(RemoteScenario::refusing("doc-unreachable"));
        let (address, notes) = (remote.address(), remote.companion("notes"));
        let mut entry = over(&remote, &["notes"], seconds, false);
        entry["refreshing"] = json!(notes);
        let scratch = Scratch::holding(&remote, &entry);
        let before = scratch.bytes();
        let given = access_token(&scratch.file, &address, None, TIMEOUT);
        if seconds == 3600 {
            assert_eq!(given.as_deref(), Ok(entry["accessToken"].as_str().unwrap()));
        } else {
            let endpoint = format!("{}/auth/token", remote.origin());
            let unreachable = matches!(&given, Err(RemoteError::Unreachable { address, .. })
                if *address == endpoint);
            assert!(unreachable, "{given:?}");
        }
        assert_eq!(scratch.bytes(), before, "{seconds}: the record stays");
    }
}

#[test]
fn a_record_that_is_not_a_string_sends_no_refresh_and_reads_as_lapsed() {
    let remote = listing_notes(RemoteScenario::accepting("doc-unread-record"));
    let (address, notes) = (remote.address(), remote.companion("notes"));
    let mut entry = over(&remote, &["notes"], 60, true);
    entry["refreshing"] = json!({ "resource": notes });
    let scratch = Scratch::holding(&remote, &entry);
    let before = scratch.bytes();
    for resource in [None, Some(notes.as_str())] {
        let refused = access_token(&scratch.file, &address, resource, TIMEOUT).unwrap_err();
        let unread = lapsed(&address, "a recorded refresh does not read");
        assert_eq!(refused, unread, "{resource:?}");
        let message = refused.to_string();
        let says = message.contains("has lapsed") && message.contains("loft remote login");
        assert!(says, "{message}");
    }
    assert!(remote.requests().is_empty(), "no refresh is sent");
    assert_eq!(scratch.bytes(), before);
}

#[test]
fn a_sign_in_kept_for_another_origin_is_neither_refreshed_nor_sent_though_it_records_a_refresh() {
    let remote = listing_notes(RemoteScenario::accepting("doc-elsewhere-record"));
    let (address, notes) = (remote.address(), remote.companion("notes"));
    let elsewhere = "http://127.0.0.1:9/documents";
    let records = [json!(notes), json!({ "resource": notes })];
    let cases = records
        .iter()
        .flat_map(|record| [(record, 60), (record, 3600)]);
    for (record, seconds) in cases {
        let mut entry = over(&remote, &["notes"], seconds, true);
        entry["resource"] = json!(elsewhere);
        entry["refreshing"] = record.clone();
        let scratch = Scratch::holding(&remote, &entry);
        let before = scratch.bytes();
        for resource in [None, Some(notes.as_str())] {
            let refused = access_token(&scratch.file, &address, resource, TIMEOUT);
            let kept_elsewhere = RemoteError::SignInElsewhere {
                address: address.clone(),
                resource: elsewhere.to_owned(),
            };
            assert_eq!(
                refused,
                Err(kept_elsewhere),
                "{record} {seconds} {resource:?}"
            );
        }
        assert!(
            remote.requests().is_empty(),
            "{record} {seconds}: no request"
        );
        assert_eq!(
            scratch.bytes(),
            before,
            "{record} {seconds}: nothing is written"
        );
    }
}
