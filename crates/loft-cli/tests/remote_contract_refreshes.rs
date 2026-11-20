//! The contract suite of the refreshes of a sign-in over a remote's protected resources:
//! - a refresh that names one resource the sign-in holds, and gets a token for it alone;
//! - `invalid_target` that rotates and revokes nothing, for a resource the remote does not serve,
//!   or none, and for one that the sign-in does not hold;
//! - a repeat answered only for the resource of its rotation, and a reuse for another that
//!   revokes the sign-in;
//! - `invalid_grant` that rotates and revokes nothing, for an unknown refresh token, another
//!   client's or a revoked sign-in's;
//! - a sign-in made for a resource that the remote no longer serves.
//!
//! A refresh that names more than one resource has a suite of its own,
//! `remote_contract_token_requests.rs`.
//!
//! Each scenario runs against the simulator and, on demand, against the remote at
//! `LOFT_REMOTE_CONTRACT_ADDRESS`. A real half spends the sign-in that `loft remote login` kept in
//! the `loft/remotes.json` that `LOFT_REMOTE_CONTRACT_SIGN_IN` names. Run each real half alone,
//! after a fresh sign-in made as its own comment says, or with a kept one no older than its
//! comment allows, with `cargo nextest run -p loft-cli --run-ignored only -E 'test(/<its name>/)'`.
//! A real half reads the kept sign-in's token endpoint, client, resource, access token and
//! refresh token, and when that access token expires where it presents it, learns the companion
//! resources from the remote's own metadata, registers a client of its own where its scenario
//! needs another client, and shares its scenario with its simulator half, so that the two cannot
//! drift. A real half whose premise does not hold fails with a message that names the premise;
//! none passes without checking. The simulator alone is asked what a real remote does not tell:
//! which access tokens a resource admits, how many refreshes it accepted, and a sign-in seeded
//! without a browser. A scenario that cannot do without them has no real half.

use serde_json::{Value, json};
use software_plant::remote::{Companion, Remote, RemoteScenario};

use crate::common::calls::Answer;
use crate::common::oauth::{
    KEY, UNSERVED, assert_oauth_error, assert_same_answer, challenge_at, companion_metadata,
    discover, fetch, metadata, parameter, refresh_for, registered,
};
use crate::common::resources::{
    Signed, companions_at, head_with, kept, kept_unexpired, signed_over,
};
use crate::remote_contract::assert_problem;

/// The premise of each real half that refreshes: the kept refresh token is the sign-in's current
/// one.
pub const UNREFRESHED: &str = "premise: nothing refreshed the sign-in since it was kept";

/// Requires `answer`, to a request under the document at `address` with an access token that is
/// not for the resource its metadata names, to be a refusal with the challenge of a request
/// without a token, which names the metadata of the resource served now.
fn refused_at_the_document(address: &str, answer: &Answer) {
    assert_problem(answer, 401, "unauthenticated");
    let challenge = challenge_at(address);
    let challenged = answer.header("www-authenticate").unwrap_or_default();
    assert_eq!(
        parameter(challenged, "resource_metadata"),
        parameter(&challenge, "resource_metadata"),
        "{challenged}"
    );
}

/// Refuses each refresh of `signed` that names a resource the remote does not serve, or none,
/// `invalid_target`, whether its refresh token is live, rotated or one never issued, and issues,
/// rotates and revokes nothing: its live refresh token then refreshes the sign-in's resource, and
/// that answer's refresh token after it.
fn refuses_a_resource_it_does_not_serve(signed: &Signed) {
    let live = signed.refresh_token.as_str();
    let refused = |resources: &[&str], token: &str| {
        let answer = refresh_for(&signed.server(), &signed.client, resources, token);
        assert_oauth_error(&answer, "invalid_target");
        let issued = answer.body.get("access_token").is_some();
        assert!(!issued, "a refusal issues no token: {answer:?}");
    };
    refused(&[UNSERVED], live);
    refused(&[], live);
    refused(&[UNSERVED], "a-refresh-token-never-issued");
    let rotated = signed.refresh(&signed.resource, live);
    let unrotated = format!("a refused refresh rotates nothing ({UNREFRESHED})");
    assert_eq!(rotated.status, 200, "{unrotated}: {rotated:?}");
    refused(&[UNSERVED], live);
    let successor = rotated.body["refresh_token"].as_str().unwrap();
    let next = signed.refresh(&signed.resource, successor);
    assert_eq!(
        next.status, 200,
        "a refused refresh revokes nothing: {next:?}"
    );
}

#[test]
fn a_refresh_for_a_resource_the_remote_does_not_serve_is_refused_and_rotates_nothing() {
    let remote = Remote::start(RemoteScenario::accepting(KEY));
    let own = remote.resource();
    refuses_a_resource_it_does_not_serve(&signed_over(&remote, &[&own], "documents"));
    assert_eq!(remote.refreshes(), 2);
    // A lapsing scenario refuses a resource it does not serve before it refuses the lapse, and
    // the lapse before it asks whether the sign-in holds the resource.
    let notes = Companion::new("notes", &["notes"]);
    let lapsing = Remote::start(
        RemoteScenario::accepting(KEY)
            .lapsing()
            .accompanied_by(notes),
    );
    let (own, notes) = (lapsing.resource(), lapsing.companion("notes"));
    let signed = signed_over(&lapsing, &[&own], "documents");
    for (resource, refused) in [
        (own.as_str(), "invalid_grant"),
        (notes.as_str(), "invalid_grant"),
        (UNSERVED, "invalid_target"),
    ] {
        let answer = signed.refresh(resource, &signed.refresh_token);
        assert_oauth_error(&answer, refused);
    }
    assert_eq!(lapsing.refreshes(), 0);
}

#[test]
#[ignore = "spends the sign-in that LOFT_REMOTE_CONTRACT_SIGN_IN keeps"]
fn a_real_remote_refuses_a_refresh_for_a_resource_it_does_not_serve_and_rotates_nothing() {
    // Needs a sign-in that any `loft remote login` kept for the address, which nothing has
    // refreshed since.
    refuses_a_resource_it_does_not_serve(&kept());
}

/// Refreshes `signed`, a sign-in that holds the companion resource `companion`, for that resource,
/// and gives the answer: an access token for it alone, which the document refuses, with a `scope`
/// of the scopes it lists, and a `resource`, where the answer holds one, that names it.
fn refreshes_a_companion_resource(signed: &Signed, companion: &str) -> Value {
    let answer = signed.refresh(companion, &signed.refresh_token);
    let premise = "premise: a sign-in that holds each companion resource: its authorization \
                   request named each, and the person consented to each";
    assert_eq!(
        answer.status, 200,
        "{premise}, {companion} among them ({UNREFRESHED}): {answer:?}"
    );
    let listed = companion_metadata(companion).body["scopes_supported"].clone();
    let body = &answer.body;
    let scope = body["scope"].as_str().unwrap_or_default();
    assert!(!scope.is_empty(), "a scope: {answer:?}");
    for granted in scope.split(' ') {
        let lists = listed
            .as_array()
            .is_some_and(|l| l.contains(&json!(granted)));
        assert!(lists, "{companion} lists {granted}: {listed}");
    }
    if let Some(resource) = body.get("resource") {
        assert_eq!(resource, &json!(companion));
    }
    let access = body["access_token"].as_str().unwrap();
    refused_at_the_document(&signed.address, &head_with(&signed.address, access));
    answer.body
}

/// Presents the refresh token of `signed` again, after `answer` rotated it for the companion
/// resource `companion`: for `companion`, it gets `answer` again, unchanged; for the sign-in's own
/// resource, it is a reuse, which revokes the sign-in, so that `answer`'s refresh token is
/// refused too.
fn repeats_only_for_its_resource(signed: &Signed, companion: &str, answer: &Value) {
    let rotated = signed.refresh_token.as_str();
    let repeated = signed.refresh(companion, rotated);
    assert_same_answer(&repeated.body, answer, &format!("a repeat for {companion}"));
    assert_oauth_error(&signed.refresh(&signed.resource, rotated), "invalid_grant");
    let successor = answer["refresh_token"].as_str().unwrap();
    let revoked = signed.refresh(&signed.resource, successor);
    assert_oauth_error(&revoked, "invalid_grant");
}

#[test]
fn one_sign_in_issues_a_token_for_each_companion_resource_and_a_reuse_for_another_revokes_it() {
    let notes = Companion::new("notes", &["notes", "notes.archive"]);
    let remote = Remote::start(RemoteScenario::accepting(KEY).accompanied_by(notes));
    let (own, notes) = (remote.resource(), remote.companion("notes"));
    let signed = signed_over(&remote, &[&own, &notes], "documents notes notes.archive");
    let answer = refreshes_a_companion_resource(&signed, &notes);
    assert_eq!(answer["scope"], "notes notes.archive");
    assert_eq!(answer["resource"], json!(notes));
    let access = answer["access_token"].as_str().unwrap();
    assert!(remote.admits_at(&notes, access) && !remote.admits_at(&own, access));
    repeats_only_for_its_resource(&signed, &notes, &answer);
    assert_eq!(remote.refreshes(), 1);
    assert!(!remote.admits_at(&notes, access));
    assert!(!remote.admits_at(&own, &signed.access_token));
}

#[test]
#[ignore = "spends the sign-in that LOFT_REMOTE_CONTRACT_SIGN_IN keeps"]
fn a_real_remote_issues_a_token_for_each_companion_resource_and_revokes_a_sign_in_reused_for_another()
 {
    // Needs a remote that lists a companion resource, and a sign-in kept for the address, which
    // nothing has refreshed since, that holds each listed companion resource: its authorization
    // request named each, and the person consented to each. The run revokes it.
    let signed = kept();
    let companions = companions_at(&signed.address);
    let (first, _) = companions
        .first()
        .expect("premise: the remote lists a companion resource");
    let answer = refreshes_a_companion_resource(&signed, first);
    repeats_only_for_its_resource(&signed, first, &answer);
}

/// Refuses a refresh of `signed`, a sign-in made for the remote's own resource alone, for the
/// companion resource `companion` `invalid_target`, which rotates and revokes nothing; once its
/// refresh token has rotated for its own resource, presenting it for `companion` is a reuse,
/// which revokes the sign-in, whose live refresh token is then refused `invalid_grant` for
/// either resource.
fn refuses_a_companion_resource_it_does_not_hold(signed: &Signed, companion: &str) {
    let live = signed.refresh_token.as_str();
    let unheld = signed.refresh(companion, live);
    let premise = "premise: the sign-in was made for the remote's own resource alone";
    assert_ne!(unheld.status, 200, "{premise}: {unheld:?}");
    assert_oauth_error(&unheld, "invalid_target");
    let rotated = signed.refresh(&signed.resource, live);
    let unrotated = format!("a refused refresh rotates nothing ({UNREFRESHED})");
    assert_eq!(rotated.status, 200, "{unrotated}: {rotated:?}");
    assert_oauth_error(&signed.refresh(companion, live), "invalid_grant");
    let successor = rotated.body["refresh_token"].as_str().unwrap();
    for resource in [signed.resource.as_str(), companion] {
        assert_oauth_error(&signed.refresh(resource, successor), "invalid_grant");
    }
}

#[test]
fn a_refresh_for_a_companion_resource_the_sign_in_does_not_hold_is_refused_and_rotates_nothing() {
    let notes = Companion::new("notes", &["notes"]);
    let remote = Remote::start(RemoteScenario::accepting(KEY).accompanied_by(notes));
    let (own, notes) = (remote.resource(), remote.companion("notes"));
    let signed = signed_over(&remote, &[&own], "documents");
    refuses_a_companion_resource_it_does_not_hold(&signed, &notes);
    assert_eq!(remote.refreshes(), 1);
    assert!(!remote.admits_at(&own, &signed.access_token));
}

#[test]
#[ignore = "spends the sign-in that LOFT_REMOTE_CONTRACT_SIGN_IN keeps"]
fn a_real_remote_refuses_a_companion_resource_that_a_sign_in_for_its_own_resource_does_not_hold() {
    // Needs a remote that lists a companion resource, and a sign-in kept for the address whose
    // authorization request named the remote's own resource alone, which nothing has refreshed
    // since. The run revokes it.
    let signed = kept();
    let companions = companions_at(&signed.address);
    let (first, _) = companions
        .first()
        .expect("premise: the remote lists a companion resource");
    refuses_a_companion_resource_it_does_not_hold(&signed, first);
}

/// Refuses `signed`, a sign-in made for a resource that the remote no longer serves: the document
/// refuses its access token, with the challenge that names the metadata of the resource served
/// now, and a refresh is refused `invalid_target` both for the resource it was made for, which
/// the remote does not serve, and for the one served now, which it does not hold.
fn refuses_a_resource_it_no_longer_serves(signed: &Signed) {
    let (_, metadata, _) = fetch(&signed.address);
    let served = metadata["resource"].as_str().unwrap_or_default();
    let moved = "premise: the remote moved its protected resource after the sign-in was made";
    assert_ne!(served, signed.resource, "{moved}");
    let answer = head_with(&signed.address, &signed.access_token);
    refused_at_the_document(&signed.address, &answer);
    let former = signed.refresh(&signed.resource, &signed.refresh_token);
    assert_oauth_error(&former, "invalid_target");
    let now = signed.refresh(served, &signed.refresh_token);
    let lapsed = format!("{UNREFRESHED}, and it has not lapsed");
    assert_ne!(now.body["error"], "invalid_grant", "{lapsed}");
    assert_oauth_error(&now, "invalid_target");
}

#[test]
fn a_sign_in_made_for_a_resource_the_remote_no_longer_serves_is_refused_at_the_document_and_on_refresh()
 {
    let remote = Remote::start(RemoteScenario::accepting(KEY));
    let former = format!("{}/former", remote.origin());
    let grant = remote.grant_over(&[&former]);
    let (_, server) = metadata(&remote);
    let signed = Signed {
        address: remote.address(),
        token_endpoint: server["token_endpoint"].as_str().unwrap().to_owned(),
        client: grant.client_id,
        resource: former.clone(),
        access_token: grant.access_token,
        refresh_token: grant.refresh_token,
    };
    refuses_a_resource_it_no_longer_serves(&signed);
    assert_eq!(remote.refreshes(), 0);
    assert!(remote.admits_at(&former, &signed.access_token));
}

#[test]
#[ignore = "spends the sign-in that LOFT_REMOTE_CONTRACT_SIGN_IN keeps"]
fn a_real_remote_refuses_a_sign_in_made_for_a_resource_it_no_longer_serves() {
    // Needs a sign-in kept for the address before the remote moved its protected resource, which
    // nothing has refreshed since it was kept, and two limits of the remote not yet reached: its
    // idle limit, counted from the sign-in's last refresh before the move, and the longest life
    // it gives a sign-in, counted from when the sign-in was made. Past either, the remote has let
    // the sign-in lapse, and the last step fails naming this premise. Its access token must not
    // have expired either: the document refuses an expired one whatever resource it was issued
    // for, so the run first fails naming that premise. The run spends a sign-in that the move
    // left of no further use.
    refuses_a_resource_it_no_longer_serves(&kept_unexpired());
}

#[test]
fn a_refresh_for_one_resource_leaves_the_tokens_of_every_other_resource_admitted() {
    let remote = Remote::start(
        RemoteScenario::accepting(KEY)
            .accompanied_by(Companion::new("notes", &["notes", "notes.archive"]))
            .accompanied_by(Companion::new("inbox", &["inbox"])),
    );
    let (own, notes) = (remote.resource(), remote.companion("notes"));
    let inbox = remote.companion("inbox");
    let signed = signed_over(&remote, &[&own, &notes, &inbox], "documents notes inbox");
    let listed = companion_metadata(&notes).body["scopes_supported"].clone();
    assert_eq!(listed, json!(["notes", "notes.archive"]));
    // Whether `access` is admitted at `resource` and at no other resource of the sign-in.
    let alone = |resource: &str, access: &str| {
        [&own, &notes, &inbox]
            .iter()
            .all(|other| remote.admits_at(other, access) == (other.as_str() == resource))
    };
    let mut issued = vec![(own.clone(), signed.access_token.clone())];
    let mut token = signed.refresh_token.clone();
    for (resource, scope) in [(&notes, "notes"), (&inbox, "inbox"), (&own, "documents")] {
        let answer = signed.refresh(resource, &token);
        assert_eq!(answer.status, 200, "{resource}: {answer:?}");
        assert_eq!(answer.body["scope"], scope, "{resource}");
        let access = answer.body["access_token"].as_str().unwrap();
        issued.push((resource.clone(), access.to_owned()));
        let unconfined: Vec<&String> = issued
            .iter()
            .filter(|(at, access)| !alone(at, access))
            .map(|(at, _)| at)
            .collect();
        assert!(
            unconfined.is_empty(),
            "after {resource}, the token issued for each of {unconfined:?} is no longer admitted \
             at that resource alone"
        );
        token = answer.body["refresh_token"].as_str().unwrap().to_owned();
    }
    assert_eq!(remote.refreshes(), 3);
    let again = signed.refresh(&notes, &token);
    assert_eq!(again.status, 200, "{again:?}");
    assert_eq!(again.body["scope"], "notes");
}

#[test]
fn a_sign_in_granted_without_a_browser_holds_an_access_token_for_each_resource_alone_and_refreshes_each()
 {
    let remote = Remote::start(
        RemoteScenario::accepting(KEY)
            .accompanied_by(Companion::new("notes", &["notes", "notes.archive"]))
            .accompanied_by(Companion::new("inbox", &["inbox"])),
    );
    let (own, notes) = (remote.resource(), remote.companion("notes"));
    let inbox = remote.companion("inbox");
    let held = [&own, &notes, &inbox];
    let grant = remote.grant_over(&[&own, &notes, &inbox]);
    let granted: Vec<&String> = grant.access.keys().collect();
    assert_eq!(granted.len(), held.len(), "{granted:?}");
    let own_token = grant.access[&own] == grant.access_token;
    assert!(own_token, "the sign-in's access token is the one for {own}");
    for (resource, access) in &grant.access {
        let alone = held
            .iter()
            .all(|other| remote.admits_at(other, access) == (*other == resource));
        assert!(
            alone,
            "the access token for {resource} is admitted there alone"
        );
    }
    // Its refresh token refreshes each resource the sign-in holds, with the scopes it was
    // granted that the resource lists, and each answer's refresh token the next.
    let (_, server) = metadata(&remote);
    let mut token = grant.refresh_token.clone();
    for (resource, scope) in [
        (&notes, "notes notes.archive"),
        (&inbox, "inbox"),
        (&own, "documents"),
    ] {
        let answer = refresh_for(&server, &grant.client_id, &[resource.as_str()], &token);
        assert_eq!(answer.status, 200, "{resource}: {answer:?}");
        assert_eq!(answer.body["scope"], scope, "{resource}");
        token = answer.body["refresh_token"].as_str().unwrap().to_owned();
    }
}

/// Refuses `invalid_grant` each refresh, for the sign-in's own resource and for the companion
/// resource `companion` alike, whether `signed` holds it or not, that presents a refresh token
/// never issued, a refresh token of `signed` by another client, or one of `signed` once a reuse
/// has revoked it; and rotates and revokes nothing: the live refresh token still refreshes, and
/// so does its successor, until that reuse.
fn refuses_an_unknown_another_client_s_or_a_revoked_sign_in_s_refresh_token(
    signed: &Signed,
    companion: &str,
) {
    let (_, server) = discover(&signed.address);
    let other = registered(&server);
    let by_other = |resource: &str, token: &str| refresh_for(&server, &other, &[resource], token);
    // Each is refused for its refresh token, before the sign-in is asked whether it holds the
    // resource.
    let own = signed.resource.as_str();
    let both = [own, companion];
    let first = signed.refresh_token.as_str();
    for resource in both {
        let unknown = signed.refresh(resource, "a-refresh-token-never-issued");
        assert_oauth_error(&unknown, "invalid_grant");
        assert_oauth_error(&by_other(resource, first), "invalid_grant");
    }
    let rotated = signed.refresh(own, first);
    let unrotated = format!("a refused refresh rotates nothing ({UNREFRESHED})");
    assert_eq!(rotated.status, 200, "{unrotated}: {rotated:?}");
    // Presented by another client within the grace window, the rotated token is refused, not
    // answered again, and revokes nothing: its successor still refreshes.
    assert_oauth_error(&by_other(own, first), "invalid_grant");
    let second = rotated.body["refresh_token"].as_str().unwrap();
    let next = signed.refresh(own, second);
    assert_eq!(
        next.status, 200,
        "a refused refresh revokes nothing: {next:?}"
    );
    // A reuse revokes the sign-in. Then its live refresh token is refused, for either resource,
    // and so is its rotated one within the grace window.
    assert_oauth_error(&signed.refresh(own, first), "invalid_grant");
    let third = next.body["refresh_token"].as_str().unwrap();
    for resource in both {
        assert_oauth_error(&signed.refresh(resource, third), "invalid_grant");
    }
    assert_oauth_error(&signed.refresh(own, second), "invalid_grant");
}

#[test]
fn an_unknown_refresh_token_another_client_s_or_a_revoked_sign_in_s_is_refused_and_changes_nothing()
{
    let notes = Companion::new("notes", &["notes"]);
    let remote = Remote::start(RemoteScenario::accepting(KEY).accompanied_by(notes));
    let (own, notes) = (remote.resource(), remote.companion("notes"));
    let signed = signed_over(&remote, &[&own], "documents");
    refuses_an_unknown_another_client_s_or_a_revoked_sign_in_s_refresh_token(&signed, &notes);
    // The two refreshes it accepted are all that rotated.
    assert_eq!(remote.refreshes(), 2);
}

#[test]
#[ignore = "spends the sign-in that LOFT_REMOTE_CONTRACT_SIGN_IN keeps"]
fn a_real_remote_refuses_an_unknown_refresh_token_another_client_s_or_a_revoked_sign_in_s() {
    // Needs a remote that lists a companion resource, and a sign-in kept for the address, which
    // nothing has refreshed since. The run registers a client and revokes the sign-in.
    let signed = kept();
    let companions = companions_at(&signed.address);
    let (first, _) = companions
        .first()
        .expect("premise: the remote lists a companion resource");
    refuses_an_unknown_another_client_s_or_a_revoked_sign_in_s_refresh_token(&signed, first);
}
