//! The sign-ins kept in the file a caller names, here a `loft/remotes.json` in a scratch
//! directory of the test's own, never the person's, run against the remote simulator: an access
//! token for each protected resource alone, all from the one refresh token.

use std::thread;
use std::time::Duration;

use serde_json::{Value, json};
use software_plant::remote::{Companion, Remote, RemoteScenario};

use super::fixture::{Scratch, TIMEOUT, listing_notes, over, refresh_token, sent};
use super::{SignIn, access_token, expires_at, forget, save};
use crate::escaped::{ESCAPED, HOSTILE, shown};
use crate::remote::RemoteError;

/// The members of `object`, sorted.
fn members(object: &Value) -> Vec<&str> {
    let mut members: Vec<&str> = object
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    members.sort_unstable();
    members
}

/// That `error` names `address` and `resource` and says what `says`.
fn assert_names(error: &RemoteError, address: &str, resource: &str, says: &[&str]) {
    let message = error.to_string();
    for said in [address, resource].iter().chain(says) {
        assert!(message.contains(said), "{said}: {message}");
    }
}

#[test]
fn a_sign_in_is_kept_in_the_file_the_caller_names_with_its_lock_beside_it() {
    let scratch = Scratch::new();
    let file = &scratch.file;
    let address = "https://remote.example/documents/one";
    let sign_in = SignIn {
        token_endpoint: "https://remote.example/auth/token".to_owned(),
        resource: "https://remote.example/documents".to_owned(),
        client_id: "client-1".to_owned(),
        access_token: "access-1".to_owned(),
        expires_at: expires_at(3600),
        refresh_token: "refresh-1".to_owned(),
        issuer: None,
        resources: Vec::new(),
    };
    save(file, address, &sign_in).unwrap();
    let lock = file.parent().unwrap().join("remotes.json.lock");
    assert!(lock.exists(), "the lock lies beside it");
    let written: Value = serde_json::from_slice(&scratch.bytes()).unwrap();
    assert_eq!(written["remotes"][address]["refreshToken"], "refresh-1");
    // An access token that expires later than five minutes ahead is given as kept, with no
    // request.
    let timeout = Duration::from_secs(1);
    let token = access_token(file, address, None, timeout);
    assert_eq!(token.as_deref(), Ok("access-1"));
    assert_eq!(forget(file, address), Ok(true));
    let none = RemoteError::NoSignIn {
        address: address.to_owned(),
    };
    assert_eq!(access_token(file, address, None, timeout), Err(none));
    assert_eq!(forget(file, address), Ok(false));
}

#[test]
fn a_companion_resource_s_first_token_comes_from_a_refresh_that_names_it_alone() {
    let remote = listing_notes(RemoteScenario::accepting("doc-first-token"));
    let (address, notes) = (remote.address(), remote.companion("notes"));
    let entry = over(&remote, &["notes"], 3600, false);
    let scratch = Scratch::holding(&remote, &entry);
    let token = access_token(&scratch.file, &address, Some(&notes), TIMEOUT).unwrap();
    assert!(
        remote.admits_at(&notes, &token),
        "a token for the companion resource"
    );
    assert!(!remote.admits(&token), "for it alone");
    assert_eq!(remote.refreshes(), 1);
    let named = vec![notes.clone()];
    assert_eq!(sent(&remote), [(refresh_token(&entry), named)]);
    let now = scratch.kept(&remote);
    let covered = &now["resources"][0];
    let shape = ["accessToken", "expiresAt", "kind", "resource"];
    assert_eq!(members(covered), shape, "{covered}");
    assert_eq!(covered["resource"], notes.as_str());
    assert_eq!(covered["kind"], "notes");
    assert_eq!(covered["accessToken"], token.as_str());
    let expires = covered["expiresAt"].as_str().unwrap();
    assert!(expires.ends_with('Z'), "{expires}");
    assert_ne!(
        now["refreshToken"], entry["refreshToken"],
        "the refresh token rotated"
    );
    // The remote's own token is kept as it was, and still admitted.
    for kept in ["accessToken", "expiresAt", "issuer", "resource"] {
        assert_eq!(now[kept], entry[kept], "{kept}");
    }
    assert!(remote.admits(now["accessToken"].as_str().unwrap()));
    assert!(now.get("refreshing").is_none(), "{now}");
}

#[test]
fn a_live_access_token_for_a_companion_resource_is_used_without_a_refresh() {
    let remote = listing_notes(RemoteScenario::accepting("doc-live-companion"));
    let (address, notes) = (remote.address(), remote.companion("notes"));
    let entry = over(&remote, &["notes"], 3600, true);
    let scratch = Scratch::holding(&remote, &entry);
    let before = scratch.bytes();
    let token = access_token(&scratch.file, &address, Some(&notes), TIMEOUT);
    assert_eq!(
        token.as_deref(),
        Ok(entry["resources"][0]["accessToken"].as_str().unwrap())
    );
    assert!(remote.requests().is_empty(), "no refresh");
    assert_eq!(scratch.bytes(), before, "nothing is written");
}

#[test]
fn a_companion_resource_the_sign_in_does_not_list_is_not_covered_and_nothing_is_asked() {
    let remote = listing_notes(RemoteScenario::accepting("doc-not-listed"));
    let (address, notes) = (remote.address(), remote.companion("notes"));
    // One sign-in in the shape of an earlier `loft` lists none; another lists one on another
    // origin than the address, which it asks for.
    let elsewhere = "http://127.0.0.1:9/notes";
    let mut listing_elsewhere = over(&remote, &["notes"], 3600, true);
    listing_elsewhere["resources"][0]["resource"] = json!(elsewhere);
    let cases = [
        (over(&remote, &[], 3600, false), notes.as_str()),
        (listing_elsewhere, elsewhere),
    ];
    for (entry, asked) in cases {
        let scratch = Scratch::holding(&remote, &entry);
        let before = scratch.bytes();
        let refused = access_token(&scratch.file, &address, Some(asked), TIMEOUT).unwrap_err();
        let not_covered = RemoteError::NotCovered {
            address: address.clone(),
            resource: asked.to_owned(),
        };
        assert_eq!(refused, not_covered, "{asked}");
        let says = [
            "does not cover",
            "`loft remote login` signs in again to reach it",
        ];
        assert_names(&refused, &address, asked, &says);
        assert!(remote.requests().is_empty(), "{asked}: nothing is asked");
        assert_eq!(scratch.bytes(), before, "{asked}: nothing is written");
    }
}

#[test]
fn a_companion_refresh_refused_invalid_target_is_not_covered_and_the_sign_in_serves_its_other_resources()
 {
    let remote = Remote::start(
        RemoteScenario::accepting("doc-partial-consent")
            .accompanied_by(Companion::new("notes", &["notes"]))
            .accompanied_by(Companion::new("inbox", &["inbox"])),
    );
    let (address, own) = (remote.address(), remote.resource());
    let (notes, inbox) = (remote.companion("notes"), remote.companion("inbox"));
    // The person consented to the remote's resource and the inbox alone, though the sign-in
    // lists notes too.
    let mut entry = over(&remote, &["inbox"], 60, false);
    let listed_inbox = entry["resources"][0].clone();
    entry["resources"] = json!([{ "resource": notes, "kind": "notes" }, listed_inbox]);
    let scratch = Scratch::holding(&remote, &entry);
    let refused = access_token(&scratch.file, &address, Some(&notes), TIMEOUT).unwrap_err();
    let not_covered = RemoteError::NotCovered {
        address: address.clone(),
        resource: notes.clone(),
    };
    assert_eq!(refused, not_covered);
    let says = [
        "does not cover",
        "`loft remote login` signs in again to reach it",
    ];
    assert_names(&refused, &address, &notes, &says);
    let first = refresh_token(&entry);
    assert_eq!(sent(&remote), [(first.clone(), vec![notes.clone()])]);
    assert_eq!(
        scratch.kept(&remote),
        entry,
        "its record removed, the sign-in is as it was"
    );
    // The refused refresh rotated nothing: the remote's own resource and the inbox are reached
    // with the sign-in as before.
    let mine = access_token(&scratch.file, &address, None, TIMEOUT).unwrap();
    assert!(remote.admits(&mine));
    let theirs = access_token(&scratch.file, &address, Some(&inbox), TIMEOUT).unwrap();
    assert!(remote.admits_at(&inbox, &theirs));
    assert_eq!(remote.refreshes(), 2);
    let refreshes = sent(&remote);
    assert_eq!(refreshes[1], (first, vec![own]));
    assert_eq!(refreshes[2].1, [inbox]);
}

#[test]
fn two_threads_that_need_two_resources_refresh_one_after_the_other() {
    // A slow refresh holds the first thread while the second starts, and a refresh token
    // presented again would revoke the sign-in, so two refreshes that overlapped could not go
    // unseen. Taking the lock away makes this test fail.
    let remote = listing_notes(
        RemoteScenario::accepting("doc-two-threads")
            .delaying_refreshes(Duration::from_millis(500))
            .without_grace(),
    );
    let (address, own, notes) = (
        remote.address(),
        remote.resource(),
        remote.companion("notes"),
    );
    let entry = over(&remote, &["notes"], 60, false);
    let scratch = Scratch::holding(&remote, &entry);
    let file = &scratch.file;
    let (mine, theirs) = thread::scope(|scope| {
        let mine = scope.spawn(|| access_token(file, &address, None, TIMEOUT));
        let theirs = scope.spawn(|| access_token(file, &address, Some(&notes), TIMEOUT));
        (mine.join().unwrap(), theirs.join().unwrap())
    });
    let (mine, theirs) = (mine.unwrap(), theirs.unwrap());
    assert!(remote.admits_at(&own, &mine), "the remote's own token");
    assert!(
        remote.admits_at(&notes, &theirs),
        "the companion resource's token"
    );
    assert_eq!(remote.refreshes(), 2);
    let refreshes = sent(&remote);
    assert_eq!(refreshes.len(), 2, "{refreshes:?}");
    assert_eq!(
        refreshes[0].0,
        refresh_token(&entry),
        "the first presents the kept token"
    );
    assert_ne!(
        refreshes[1].0, refreshes[0].0,
        "the second presents the first's successor"
    );
    let mut named: Vec<&str> = refreshes
        .iter()
        .map(|(_, named)| named[0].as_str())
        .collect();
    named.sort_unstable();
    let mut both = [own.as_str(), notes.as_str()];
    both.sort_unstable();
    assert_eq!(named, both);
    let now = scratch.kept(&remote);
    assert_eq!(now["accessToken"], mine.as_str());
    assert_eq!(now["resources"][0]["accessToken"], theirs.as_str());
    assert!(now.get("refreshing").is_none(), "{now}");
}

#[test]
fn a_refresh_refused_invalid_target_for_the_sign_in_s_own_resource_says_it_was_made_for_a_former_resource()
 {
    let remote = Remote::start(RemoteScenario::accepting("doc-former"));
    let address = remote.address();
    let former = format!("{}/former", remote.origin());
    let grant = remote.grant_over(&[&former]);
    let entry = json!({
        "tokenEndpoint": format!("{}/auth/token", remote.origin()),
        "resource": former,
        "clientId": grant.client_id,
        "accessToken": grant.access_token,
        "expiresAt": expires_at(60),
        "refreshToken": grant.refresh_token,
    });
    let scratch = Scratch::holding(&remote, &entry);
    let before = scratch.bytes();
    let refused = access_token(&scratch.file, &address, None, TIMEOUT).unwrap_err();
    let made_for_former = RemoteError::FormerResource {
        address: address.clone(),
        resource: former.clone(),
    };
    assert_eq!(refused, made_for_former);
    let says = ["former resource", "`loft remote login` signs in again"];
    assert_names(&refused, &address, &former, &says);
    assert_eq!(sent(&remote), [(grant.refresh_token, vec![former])]);
    assert_eq!(scratch.bytes(), before, "the sign-in is left as it was");
}

#[test]
fn a_companion_resource_the_remote_refuses_is_named_escaped_as_one_the_sign_in_does_not_cover() {
    let remote = listing_notes(RemoteScenario::accepting("doc-hostile-companion"));
    let address = remote.address();
    // A companion resource kept in `resources` whose fragment the remote gave, which the remote
    // does not serve.
    let listed = format!("{}#{HOSTILE}", remote.companion("notes"));
    let mut entry = over(&remote, &["notes"], 60, false);
    entry["resources"][0]["resource"] = json!(listed);
    let scratch = Scratch::holding(&remote, &entry);
    let refused = access_token(&scratch.file, &address, Some(&listed), TIMEOUT).unwrap_err();
    let not_covered = RemoteError::NotCovered {
        address: address.clone(),
        resource: listed,
    };
    assert_eq!(refused, not_covered);
    let message = refused.to_string();
    let named = format!("does not cover {}#{ESCAPED}", remote.companion("notes"));
    assert!(message.contains(&named), "{message}");
    assert!(shown(&message), "{message:?}");
}

#[test]
fn an_entry_that_is_not_an_object_is_no_sign_in_and_is_neither_refreshed_nor_sent() {
    let remote = Remote::start(RemoteScenario::accepting("doc-not-an-object"));
    let address = remote.address();
    let none = RemoteError::NoSignIn {
        address: address.clone(),
    };
    // A list of a sign-in's values in the order of its members: with a minute left its access
    // token would be refreshed, and with an hour left sent.
    let members = [
        "tokenEndpoint",
        "resource",
        "clientId",
        "accessToken",
        "expiresAt",
        "refreshToken",
    ];
    let lists = [60, 3600].map(|seconds| {
        let entry = over(&remote, &[], seconds, false);
        let values = members.iter().map(|member| entry[*member].clone());
        Value::Array(values.collect())
    });
    for entry in lists.into_iter().chain([json!("refresh-1"), json!(null)]) {
        let scratch = Scratch::holding(&remote, &entry);
        let before = scratch.bytes();
        let token = access_token(&scratch.file, &address, None, TIMEOUT);
        assert_eq!(token, Err(none.clone()), "{entry}");
        assert!(
            remote.requests().is_empty(),
            "{entry}: nothing is refreshed"
        );
        assert_eq!(scratch.bytes(), before, "{entry}: nothing is written");
    }
}
