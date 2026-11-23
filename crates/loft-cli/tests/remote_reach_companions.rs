//! A sign-in over a remote's companion resources as connected commands use it: the remote's own
//! token alone on each request, a refresh that keeps every other resource's token, the record of
//! a refresh whose answer is lost, a recorded refresh that the remote refuses, a sign-in made for
//! a former resource, and the members a refresh does not replace, run under the remote and git
//! simulators; and the words `loft interface status` gives a sign-in made for a former resource,
//! and a recorded refresh refused, at another document's remote.

use std::fs;
use std::process::Output;

use loftspec::publish::PublishedAt;
use serde_json::{Value, json};
use software_plant::remote::wire::{pairs, value};
use software_plant::remote::{Companion, RESOURCE, Remote, RemoteScenario};
use software_plant::{BrowserScenario, InstalledBrowser};

use crate::common::connected::{Connected, connected};
use crate::common::copies::HANDLE;
use crate::common::oauth::{metadata, refresh};
use crate::common::others::{listing, shelves, shelves_scenario};
use crate::common::signed_in::{
    self, granted, granted_over, keep, real_now, sign_ins, sign_ins_file,
};
use crate::common::{Scratch, tree};
use crate::interface_add::{conforming, interface};

/// A connected repository whose remote answers as `scenario` and lists the companion resource
/// of the kind `notes`.
fn listing_notes(scenario: impl FnOnce(&str) -> RemoteScenario) -> Connected {
    connected(1, 1, |key| {
        scenario(key).accompanied_by(Companion::new("notes", &["notes"]))
    })
}

/// Keeps `sign_in` as the sign-in to the remote of `run`.
fn keeping(run: &Connected, sign_in: &Value) {
    keep(&run.scratch, &json!({ run.remote.address(): sign_in }));
}

/// The sign-in kept for the remote of `run`.
fn kept(run: &Connected) -> Value {
    sign_ins(&run.scratch)[run.remote.address()].clone()
}

/// `sign_in` without its record of a refresh.
fn unrecorded(mut sign_in: Value) -> Value {
    sign_in.as_object_mut().unwrap().remove("refreshing");
    sign_in
}

/// The refresh token that `sign_in` presents.
fn refresh_token(sign_in: &Value) -> String {
    sign_in["refreshToken"].as_str().unwrap().to_owned()
}

/// The form of each request to the token endpoint of the remote of `run`, in order.
fn token_requests(run: &Connected) -> Vec<Vec<(String, String)>> {
    let requests = run.remote.requests().into_iter();
    let to_token = requests.filter(|request| request.path == "/auth/token");
    to_token
        .map(|request| pairs(&String::from_utf8_lossy(&request.body)))
        .collect()
}

/// Each refresh that the remote of `run` received, in order: the refresh token it presented and
/// each `resource` it named.
fn refreshes(run: &Connected) -> Vec<(String, Vec<String>)> {
    let forms = token_requests(run).into_iter();
    let refreshing = forms.filter(|form| value(form, "grant_type") == Some("refresh_token"));
    refreshing
        .map(|form| {
            let presented = value(&form, "refresh_token").unwrap_or_default().to_owned();
            let named = form.iter().filter(|(field, _)| field == "resource");
            (
                presented,
                named.map(|(_, resource)| resource.clone()).collect(),
            )
        })
        .collect()
}

/// A sign-in to `remote` as `loft` keeps it, made for a protected resource on its origin that the
/// remote no longer serves, its access token expiring in a minute.
fn made_for_former(remote: &Remote) -> Value {
    let former = format!("{}/former", remote.origin());
    let grant = remote.grant_over(&[&former]);
    let expires = PublishedAt::from_unix_seconds(real_now() + 60).unwrap();
    json!({
        "tokenEndpoint": format!("{}/auth/token", remote.origin()),
        "resource": former,
        "clientId": grant.client_id,
        "accessToken": grant.access_token,
        "expiresAt": expires.to_string(),
        "refreshToken": grant.refresh_token,
    })
}

/// `loft publish` over the tree of `run`, with a browser that visits any address it is given.
fn publish_visiting(run: &Connected) -> (Output, InstalledBrowser) {
    let (mut command, browser) =
        signed_in::command(&run.scratch, BrowserScenario::Visiting, Some(&run.git));
    let output = command.args(["publish", "--format", "json"]).output();
    (output.unwrap(), browser)
}

/// `loft remote set` with the address of the remote of `run`, with a browser that visits any
/// address it is given.
fn set_visiting(run: &Connected) -> (Output, InstalledBrowser) {
    let address = run.remote.address();
    let args = ["remote", "set", address.as_str()];
    signed_in::run(&run.scratch, &args, BrowserScenario::Visiting, None)
}

/// That the message of the error that stopped `output` holds each of `said`.
fn assert_says(output: &Output, said: &[&str]) {
    let message = Connected::message(output);
    for said in said {
        assert!(message.contains(said), "{said}: {message}");
    }
}

#[test]
fn a_publish_with_a_sign_in_over_companion_resources_sends_the_remote_s_own_token_alone() {
    let run = listing_notes(RemoteScenario::accepting);
    let sign_in = granted_over(&run.remote, &["notes"], 3600);
    keeping(&run, &sign_in);
    run.change();
    let output = run.publish();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(run.remote.refreshes(), 0);
    assert_eq!(run.publish_requests().len(), 1);
    let mine = format!("Bearer {}", sign_in["accessToken"].as_str().unwrap());
    let theirs = sign_in["resources"][0]["accessToken"].as_str().unwrap();
    for request in run.remote.requests() {
        let carried = request.header("authorization");
        let path = &request.path;
        assert!(
            carried.is_none_or(|carried| carried == mine),
            "{path}: {carried:?}"
        );
        let mut headers = request.headers.iter();
        assert!(!headers.any(|(_, held)| held.contains(theirs)), "{path}");
        let body = String::from_utf8_lossy(&request.body);
        assert!(!body.contains(theirs) && !path.contains(theirs), "{path}");
    }
}

#[test]
fn a_refresh_of_the_remote_s_own_token_keeps_each_companion_token_and_leaves_no_record() {
    let run = listing_notes(RemoteScenario::accepting);
    let sign_in = granted_over(&run.remote, &["notes"], 60);
    keeping(&run, &sign_in);
    run.change();
    let output = run.publish();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(run.remote.refreshes(), 1);
    let own = run.remote.resource();
    assert_eq!(
        refreshes(&run),
        [(refresh_token(&sign_in), vec![own.clone()])]
    );
    let to_token = run.remote.requests().into_iter();
    for request in to_token.filter(|request| request.path == "/auth/token") {
        assert_eq!(
            request.header("authorization"),
            None,
            "a refresh carries no token"
        );
    }
    // `resource`, `accessToken` and `expiresAt` stay the remote's own, and `refreshToken` the
    // sign-in's one, which the companion resource's refresh presents too.
    let now = kept(&run);
    assert_eq!(now["resource"], own.as_str());
    let mine = now["accessToken"].as_str().unwrap();
    assert_ne!(now["accessToken"], sign_in["accessToken"]);
    assert!(run.remote.admits(mine), "{now}");
    assert_ne!(now["expiresAt"], sign_in["expiresAt"]);
    assert_ne!(now["refreshToken"], sign_in["refreshToken"]);
    let publish = run.publish_requests().remove(0);
    let bearer = format!("Bearer {mine}");
    assert_eq!(publish.header("authorization"), Some(bearer.as_str()));
    assert_eq!(
        now["resources"], sign_in["resources"],
        "the companion's token is kept"
    );
    assert_eq!(now["issuer"], sign_in["issuer"]);
    assert!(now.get("refreshing").is_none(), "no record is left: {now}");
    let notes = run.remote.companion("notes");
    let theirs = now["resources"][0]["accessToken"].as_str().unwrap();
    assert!(run.remote.admits_at(&notes, theirs));
    let (_, server) = metadata(&run.remote);
    let client = now["clientId"].as_str().unwrap();
    let refreshed = refresh(&server, client, &notes, &refresh_token(&now));
    assert_eq!(refreshed.status, 200, "{}", refreshed.body["error"]);
}

#[test]
fn a_lost_refresh_answer_is_recorded_and_the_next_publish_repeats_it_first() {
    let run = listing_notes(|key| RemoteScenario::accepting(key).losing_a_refresh_answer());
    let sign_in = granted_over(&run.remote, &["notes"], 60);
    keeping(&run, &sign_in);
    run.change();
    let lost = run.publish();
    assert_eq!(lost.status.code(), Some(1), "{lost:?}");
    let recorded = kept(&run);
    let own = run.remote.resource();
    assert_eq!(
        recorded["refreshing"],
        own.as_str(),
        "the record names the resource"
    );
    assert_eq!(unrecorded(recorded), sign_in, "the tokens are as they were");
    let again = run.publish();
    assert_eq!(again.status.code(), Some(0), "{again:?}");
    let repeated = (refresh_token(&sign_in), vec![own]);
    assert_eq!(refreshes(&run), [repeated.clone(), repeated]);
    assert_eq!(
        run.remote.refreshes(),
        1,
        "the repeat is answered as the first was"
    );
    let now = kept(&run);
    assert!(
        now.get("refreshing").is_none(),
        "the record is removed: {now}"
    );
    assert_ne!(now["refreshToken"], sign_in["refreshToken"]);
    assert!(run.remote.admits(now["accessToken"].as_str().unwrap()));
    assert_eq!(
        now["resources"], sign_in["resources"],
        "the companion's token is kept"
    );
}

#[test]
fn a_sign_in_made_for_a_former_resource_is_refused_naming_login_and_kept_as_it_was() {
    let run = connected(1, 1, RemoteScenario::accepting);
    let sign_in = made_for_former(&run.remote);
    keeping(&run, &sign_in);
    run.change();
    let before = tree(&run.scratch.0);
    let (output, browser) = publish_visiting(&run);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let former = sign_in["resource"].as_str().unwrap();
    let address = run.remote.address();
    let said = ["former resource", "`loft remote login` signs in again"];
    assert_says(&output, &[address.as_str(), former, said[0], said[1]]);
    assert_eq!(
        browser.visits(),
        Vec::<String>::new(),
        "no browser is opened"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("open this address"),
        "no port is listened on: {stderr}"
    );
    assert_eq!(kept(&run), sign_in, "the sign-in is left as it was");
    assert_eq!(tree(&run.scratch.0), before);
    assert_eq!(run.remote.refreshes(), 0);
    let refused = (refresh_token(&sign_in), vec![former.to_owned()]);
    assert_eq!(refreshes(&run), [refused]);
}

#[test]
fn remote_set_signs_in_anew_in_place_of_a_sign_in_made_for_a_former_resource() {
    let run = connected(1, 1, RemoteScenario::accepting);
    let former = made_for_former(&run.remote);
    keeping(&run, &former);
    let (output, browser) = set_visiting(&run);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(browser.visits().len(), 1, "the person signs in anew");
    let now = kept(&run);
    assert_eq!(now["resource"], run.remote.resource());
    assert!(run.remote.admits(now["accessToken"].as_str().unwrap()));
    assert_eq!(refreshes(&run)[0].1, [former["resource"].as_str().unwrap()]);
}

#[test]
fn a_refresh_keeps_each_member_of_the_kept_sign_in_that_it_does_not_replace() {
    let run = connected(1, 1, RemoteScenario::accepting);
    let mut sign_in = granted(&run.remote, 60);
    sign_in["later"] = json!("kept by a later loft");
    keeping(&run, &sign_in);
    run.change();
    let output = run.publish();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(run.remote.refreshes(), 1);
    let now = kept(&run);
    assert_eq!(now["later"], "kept by a later loft");
    assert_ne!(now["refreshToken"], sign_in["refreshToken"]);
    let members = now.as_object().unwrap().keys().map(String::as_str);
    let mut members: Vec<&str> = members.filter(|member| *member != "later").collect();
    members.sort_unstable();
    let six = [
        "accessToken",
        "clientId",
        "expiresAt",
        "refreshToken",
        "resource",
        "tokenEndpoint",
    ];
    assert_eq!(
        members, six,
        "no `issuer`, `resources` or `refreshing`: {now}"
    );
}

#[test]
fn a_recorded_refresh_the_remote_refuses_stops_a_publish_and_remote_set_signs_in_anew() {
    let run = listing_notes(RemoteScenario::accepting);
    // A resource the remote does not serve, as one it served before.
    let retired = format!("{}/retired", run.remote.origin());
    let mut sign_in = granted_over(&run.remote, &["notes"], 3600);
    sign_in["refreshing"] = json!(retired);
    keeping(&run, &sign_in);
    run.change();
    let file = fs::read(sign_ins_file(&run.scratch)).unwrap();
    let (output, browser) = publish_visiting(&run);
    assert_eq!(
        output.status.code(),
        Some(1),
        "though the own token is live: {output:?}"
    );
    let address = run.remote.address();
    let said = [address.as_str(), retired.as_str(), "does not cover"];
    assert_says(&output, &[said[0], said[1], said[2], "loft remote login"]);
    assert_eq!(
        browser.visits(),
        Vec::<String>::new(),
        "no browser is opened"
    );
    let now = fs::read(sign_ins_file(&run.scratch)).unwrap();
    assert_eq!(now, file, "loft/remotes.json is as it was");
    assert_eq!(run.remote.refreshes(), 0);
    let repeated = (refresh_token(&sign_in), vec![retired.clone()]);
    assert_eq!(refreshes(&run), [repeated], "no other refresh");
    let mut reached = run.remote.requests().into_iter();
    assert!(
        reached.all(|request| !request.path.starts_with(RESOURCE)),
        "no document call"
    );
    // `loft remote set` repeats the recorded refresh, and signs in anew in its place.
    let (output, browser) = set_visiting(&run);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(browser.visits().len(), 1, "the person signs in anew");
    let forms = token_requests(&run);
    let grants: Vec<_> = forms.iter().map(|form| value(form, "grant_type")).collect();
    let repeat_then_exchange = [Some("refresh_token"), Some("authorization_code")];
    assert_eq!(grants[1..], repeat_then_exchange);
    assert_eq!(value(&forms[1], "resource"), Some(retired.as_str()));
    let now = kept(&run);
    assert!(now.get("refreshing").is_none(), "{now}");
    assert_eq!(now["resource"], run.remote.resource());
    assert_eq!(
        now["resources"][0]["resource"],
        run.remote.companion("notes")
    );
}

/// A tree that holds a copy of an Interface of another document, whose remote lists the
/// companion resource of the kind `notes`; the sign-in that `sign_in` gives is then kept for
/// that remote in place of the one the copy was taken with.
fn copied_keeping(sign_in: impl FnOnce(&Remote) -> Value) -> (Scratch, Remote) {
    let history = shelves();
    let notes = Companion::new("notes", &["notes"]);
    let remote = Remote::start(shelves_scenario(&history).accompanied_by(notes));
    let scratch = Scratch::with_reading_list();
    listing(&scratch, &remote, true);
    let added = interface(&scratch, &["add", "shelves:IF-1"]);
    assert_eq!(added.status.code(), Some(0), "{added:?}");
    keep(&scratch, &json!({ remote.address(): sign_in(&remote) }));
    (scratch, remote)
}

/// `loft interface status` over the tree of `scratch`, which must exit 1, with a browser that
/// visits any address it is given: the message of its error, its standard error and the browser.
fn status_refused(scratch: &Scratch) -> (String, String, InstalledBrowser) {
    let args = ["interface", "status", "--format", "json"];
    let (output, browser) = signed_in::run(scratch, &args, BrowserScenario::Visiting, None);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let document = conforming(&output);
    let message = document["errors"][0]["message"]
        .as_str()
        .unwrap()
        .to_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    (message, stderr, browser)
}

#[test]
fn a_status_whose_sign_in_was_made_for_a_former_resource_names_interface_login_and_keeps_it() {
    let (scratch, remote) = copied_keeping(made_for_former);
    let file = fs::read(sign_ins_file(&scratch)).unwrap();
    let before = tree(&scratch.0);
    let (message, stderr, browser) = status_refused(&scratch);
    let (address, former) = (remote.address(), format!("{}/former", remote.origin()));
    let login = format!("`loft interface login {HANDLE}` signs in again");
    for said in [&address, &former, "former resource", &login] {
        assert!(message.contains(said), "{said}: {message}");
    }
    assert!(!message.contains("loft remote"), "{message}");
    assert_eq!(
        browser.visits(),
        Vec::<String>::new(),
        "no browser is opened"
    );
    assert!(
        !stderr.contains("open this address"),
        "no port is listened on: {stderr}"
    );
    let now = fs::read(sign_ins_file(&scratch)).unwrap();
    assert_eq!(now, file, "the sign-in is left as it was");
    assert_eq!(tree(&scratch.0), before);
    assert_eq!(remote.refreshes(), 0);
}

#[test]
fn a_status_whose_recorded_refresh_the_remote_refuses_names_the_resource_and_interface_login() {
    let retired = |remote: &Remote| format!("{}/retired", remote.origin());
    let (scratch, remote) = copied_keeping(|remote| {
        // The remote's own access token is live, and the record names a resource the remote
        // does not serve.
        let mut sign_in = granted_over(remote, &["notes"], 3600);
        sign_in["refreshing"] = json!(retired(remote));
        sign_in
    });
    let file = fs::read(sign_ins_file(&scratch)).unwrap();
    let (message, _, browser) = status_refused(&scratch);
    let (address, retired) = (remote.address(), retired(&remote));
    let login = format!("`loft interface login {HANDLE}` signs in again to reach it");
    for said in [&address, &retired, "does not cover", &login] {
        assert!(message.contains(said), "{said}: {message}");
    }
    assert!(!message.contains("loft remote"), "{message}");
    assert_eq!(
        browser.visits(),
        Vec::<String>::new(),
        "no browser is opened"
    );
    let now = fs::read(sign_ins_file(&scratch)).unwrap();
    assert_eq!(now, file, "loft/remotes.json is as it was");
    assert_eq!(remote.refreshes(), 0);
}
