//! A sign-in over the companion resources a remote lists, run end to end against the remote
//! simulator's authorization server and the browser simulator: what is read, what is left out,
//! the one authorization request and what is kept, at the document's own remote and at another
//! document's that `loft interface login` signs in to.

use std::process::Output;

use serde_json::{Value, json};
use software_plant::BrowserScenario;
use software_plant::remote::wire::pairs;
use software_plant::remote::{Companion, CompanionFault, RESOURCE, Remote, RemoteScenario};

use crate::common::Scratch;
use crate::common::copies::{HANDLE, OTHER};
use crate::common::oauth::{RESOURCE_METADATA, companion_metadata, metadata, refresh};
use crate::common::others::listing;
use crate::common::signed_in::{self, sign_ins};
use crate::remote_sign_in_refusals::login;

/// Every value of `name` in the query of `url`, in order.
pub(crate) fn every(url: &str, name: &str) -> Vec<String> {
    let query = url.split_once('?').map_or("", |(_, query)| query);
    let named = pairs(query).into_iter().filter(|(field, _)| field == name);
    named.map(|(_, value)| value).collect()
}

/// The path and query of each request that the origin of `remote` received, in order.
pub(crate) fn targets(remote: &Remote) -> Vec<String> {
    let requests = remote.requests().into_iter();
    requests.map(|request| request.path).collect()
}

/// Each line of the standard error of `output` that gives a warning.
pub(crate) fn warnings(output: &Output) -> Vec<String> {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let warned = stderr.lines().filter(|line| line.starts_with("warning:"));
    warned.map(str::to_owned).collect()
}

/// The sign-in that `loft/remotes.json` under `scratch` keeps for the address of `remote`.
pub(crate) fn kept(scratch: &Scratch, remote: &Remote) -> Value {
    sign_ins(scratch)[remote.address()].clone()
}

/// The members of `sign_in`, sorted.
fn members(sign_in: &Value) -> Vec<String> {
    let mut members: Vec<String> = sign_in.as_object().unwrap().keys().cloned().collect();
    members.sort();
    members
}

/// The warning that a sign-in to `remote` leaves out the entry of `resource` and `kind`, and why.
fn left_out(remote: &Remote, resource: &str, kind: &str, why: &str) -> String {
    let address = remote.address();
    format!("warning: the sign-in to {address} leaves out {resource} ({kind}): {why}")
}

#[test]
fn one_authorization_request_names_the_remote_s_resource_and_each_companion_resource_with_the_union_of_their_scopes()
 {
    let (remote, scratch, output, visits) = login(
        RemoteScenario::accepting("doc-companions")
            .accompanied_by(Companion::new("notes", &["notes", "notes.archive"]))
            .accompanied_by(
                Companion::new("inbox", &["inbox", "documents"]).at("/mail/inbox?view=all"),
            ),
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(visits.len(), 1, "one consent: {visits:?}");
    let (notes, inbox) = (remote.companion("notes"), remote.companion("inbox"));
    assert_eq!(
        every(&visits[0], "resource"),
        [remote.resource(), notes.clone(), inbox.clone()]
    );
    assert_eq!(
        every(&visits[0], "scope"),
        ["documents notes notes.archive inbox"]
    );
    assert!(warnings(&output).is_empty(), "{output:?}");

    // Each companion resource's metadata is read where RFC 9728 places it, once the authorization
    // server's metadata has passed its checks and before a client is registered.
    let targets = targets(&remote);
    let at = |target: &str| {
        let found = targets.iter().position(|asked| asked == target);
        found.unwrap_or_else(|| panic!("{target} in {targets:?}"))
    };
    let order = [
        at("/.well-known/oauth-authorization-server/auth"),
        at(&format!("{RESOURCE_METADATA}/notes")),
        at(&format!("{RESOURCE_METADATA}/mail/inbox?view=all")),
        at("/auth/register"),
    ];
    assert!(order.is_sorted(), "{targets:?}");
    for request in remote.requests() {
        let carried = request.header("Authorization");
        assert_eq!(carried, None, "{} carries a token", request.path);
    }

    // The code is exchanged for the remote's protected resource alone.
    let exchange = remote
        .requests()
        .into_iter()
        .find(|r| r.path == "/auth/token");
    let form = pairs(&String::from_utf8(exchange.unwrap().body).unwrap());
    let exchanged: Vec<&str> = form
        .iter()
        .filter(|(field, _)| field == "resource")
        .map(|(_, value)| value.as_str())
        .collect();
    assert_eq!(exchanged, [remote.resource()]);

    let sign_in = kept(&scratch, &remote);
    assert_eq!(sign_in["resource"], remote.resource());
    assert_eq!(sign_in["issuer"], remote.issuer());
    assert_eq!(
        sign_in["resources"],
        json!([
            { "resource": notes, "kind": "notes" },
            { "resource": inbox, "kind": "inbox" },
        ])
    );
    assert!(sign_in["expiresAt"].as_str().unwrap().ends_with('Z'));
    let access = sign_in["accessToken"].as_str().unwrap();
    assert!(remote.admits(access), "the remote's own access token");
    assert!(!remote.admits_at(&notes, access), "for the remote alone");
    // What the sign-in read: the remote's `companion_resources`, and the `scopes_supported` of
    // each companion resource's own metadata.
    let (listing, server) = metadata(&remote);
    let listed = listing["companion_resources"].as_array().map(Vec::len);
    assert_eq!(listed, Some(2), "{listing}");
    let scopes = &companion_metadata(&notes).body["scopes_supported"];
    assert_eq!(scopes, &json!(["notes", "notes.archive"]));
    // The one consent covers each companion resource, which the one refresh token reaches.
    let client = sign_in["clientId"].as_str().unwrap();
    let refreshed = refresh(
        &server,
        client,
        &notes,
        sign_in["refreshToken"].as_str().unwrap(),
    );
    assert_eq!(refreshed.status, 200, "{}", refreshed.body["error"]);
}

#[test]
fn a_companion_resource_on_another_origin_is_left_out_before_anything_is_asked_of_it() {
    let (remote, scratch, output, visits) = login(
        RemoteScenario::accepting("doc-companion-elsewhere")
            .accompanied_by(Companion::new("notes", &["notes"]).elsewhere())
            .accompanied_by(Companion::new("inbox", &["inbox"])),
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    // The other origin's port serves nothing but the metadata of the companion resource placed
    // there, so it stands for every request for it.
    let asked = remote.requests_elsewhere();
    let asked: Vec<&str> = asked.iter().map(|request| request.path.as_str()).collect();
    assert_eq!(asked, Vec::<&str>::new(), "nothing is asked of it");
    let (notes, inbox) = (remote.companion("notes"), remote.companion("inbox"));
    assert_eq!(
        every(&visits[0], "resource"),
        [remote.resource(), inbox.clone()]
    );
    assert_eq!(every(&visits[0], "scope"), ["documents inbox"]);
    let why = "it does not lie on the origin of the address";
    assert_eq!(warnings(&output), [left_out(&remote, &notes, "notes", why)]);
    assert_eq!(
        kept(&scratch, &remote)["resources"],
        json!([{ "resource": inbox, "kind": "inbox" }])
    );
}

#[test]
fn a_companion_resource_whose_metadata_does_not_vouch_for_it_is_left_out_with_a_warning() {
    let faulty = |fault| Companion::new("notes", &["notes"]).faulty(fault);
    for (notes, said) in [
        (
            faulty(CompanionFault::UnderAnotherIssuer),
            "its metadata does not name the authorization server",
        ),
        (
            faulty(CompanionFault::Misnamed),
            "its metadata names another resource",
        ),
        (
            faulty(CompanionFault::Unnamed),
            "its metadata names no resource",
        ),
        (
            faulty(CompanionFault::Scopeless),
            "its metadata lists no scope",
        ),
        (
            faulty(CompanionFault::EmptyScopes),
            "its metadata lists no scope",
        ),
        (
            faulty(CompanionFault::Unreadable),
            "answered 404 without a metadata document",
        ),
        (
            faulty(CompanionFault::NotAnObject),
            "answered 200 without a metadata document",
        ),
        (
            Companion::new("notes", &["notes", "notes\tarchive"]),
            r#"its metadata lists "notes\tarchive", which is not a scope"#,
        ),
    ] {
        let (remote, scratch, output, visits) = login(
            RemoteScenario::accepting("doc-unvouched")
                .accompanied_by(notes)
                .accompanied_by(Companion::new("inbox", &["inbox"])),
        );
        assert_eq!(output.status.code(), Some(0), "{said}: {output:?}");
        let inbox = remote.companion("inbox");
        let asked = every(&visits[0], "resource");
        assert_eq!(asked, [remote.resource(), inbox.clone()], "{said}");
        assert_eq!(every(&visits[0], "scope"), ["documents inbox"], "{said}");
        let warned = warnings(&output);
        let notes = remote.companion("notes");
        let named = left_out(&remote, &notes, "notes", "");
        assert!(
            warned.len() == 1 && warned[0].starts_with(&named) && warned[0].contains(said),
            "{said}: {warned:?}"
        );
        let resources = &kept(&scratch, &remote)["resources"];
        assert_eq!(resources, &json!([{ "resource": inbox, "kind": "inbox" }]));
    }

    // A companion resource kept for metadata that lists companion resources of its own: that
    // list is not read.
    let (remote, _scratch, output, visits) = login(
        RemoteScenario::accepting("doc-relisting")
            .accompanied_by(faulty(CompanionFault::Relisting))
            .accompanied_by(Companion::new("inbox", &["inbox"])),
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let (notes, inbox) = (remote.companion("notes"), remote.companion("inbox"));
    assert_eq!(
        every(&visits[0], "resource"),
        [remote.resource(), notes.clone(), inbox]
    );
    let targets = targets(&remote);
    let relisted = targets.iter().find(|target| target.contains("relisted"));
    assert_eq!(relisted, None, "{targets:?}");
    assert!(warnings(&output).is_empty(), "{output:?}");
    let relisting = companion_metadata(&notes).body;
    assert!(relisting["companion_resources"].is_array(), "{relisting}");
}

#[test]
fn malformed_repeated_and_own_entries_are_left_out_with_a_warning_and_nothing_is_asked_of_them() {
    let (remote, scratch, output, visits) = login(
        RemoteScenario::accepting("doc-listing")
            .accompanied_by(Companion::new("notes", &["notes"]))
            .listing(json!([
                { "resource": "{origin}/notes", "kind": "notes" },
                { "resource": "{origin}/notes", "kind": "again" },
                { "resource": "{origin}/other", "kind": "notes" },
                { "resource": format!("{{origin}}{RESOURCE}"), "kind": "own" },
                { "kind": "nameless" },
                "an entry",
                { "resource": "{origin}/part#one", "kind": "fragment" },
                { "resource": "{origin}/inbox", "kind": "" },
            ])),
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let (origin, notes) = (remote.origin(), remote.companion("notes"));
    assert_eq!(
        every(&visits[0], "resource"),
        [remote.resource(), notes.clone()]
    );
    assert_eq!(every(&visits[0], "scope"), ["documents notes"]);
    let repeated = "an entry before it names the same resource or kind";
    let unnamed = format!(
        "warning: the sign-in to {} leaves out an entry of `companion_resources` that is not an \
         object of a string `resource` and a non-empty string `kind`",
        remote.address()
    );
    assert_eq!(
        warnings(&output),
        [
            left_out(&remote, &notes, "again", repeated),
            left_out(&remote, &format!("{origin}/other"), "notes", repeated),
            left_out(
                &remote,
                &remote.resource(),
                "own",
                "it is the remote's own protected resource"
            ),
            unnamed.clone(),
            unnamed.clone(),
            left_out(
                &remote,
                &format!("{origin}/part#one"),
                "fragment",
                "its identifier holds user information or a fragment"
            ),
            unnamed,
        ]
    );
    // No request is sent for an entry left out: the remote's metadata and the companion
    // resource's are each read once, nothing else under the well-known path, and nothing at all
    // of an entry left out.
    let targets = targets(&remote);
    let read: Vec<&String> = targets
        .iter()
        .filter(|target| target.starts_with(RESOURCE_METADATA))
        .collect();
    let own = format!("{RESOURCE_METADATA}{RESOURCE}");
    let companion = format!("{RESOURCE_METADATA}/notes");
    assert_eq!(read, [&own, &companion], "{targets:?}");
    let asked = |path: &str| targets.iter().any(|target| target.contains(path));
    assert!(
        !["/other", "/part", "/inbox"].into_iter().any(asked),
        "{targets:?}"
    );
    assert_eq!(
        kept(&scratch, &remote)["resources"],
        json!([{ "resource": notes, "kind": "notes" }])
    );
}

#[test]
fn a_listing_that_is_no_list_reads_as_none_with_a_warning_and_an_empty_one_as_none_without() {
    let listing = json!({ "resource": "{origin}/notes", "kind": "notes" });
    for (listed, warned) in [(listing, true), (json!([]), false)] {
        let (remote, scratch, output, visits) =
            login(RemoteScenario::accepting("doc-no-list").listing(listed.clone()));
        assert_eq!(output.status.code(), Some(0), "{listed}: {output:?}");
        assert_eq!(every(&visits[0], "resource"), [remote.resource()]);
        assert_eq!(every(&visits[0], "scope"), ["documents"]);
        let no_list = format!(
            "warning: the sign-in to {} reads no companion resource: `companion_resources` is not \
             a list",
            remote.address()
        );
        let expected = if warned { vec![no_list] } else { Vec::new() };
        assert_eq!(warnings(&output), expected, "{listed}");
        let six = [
            "accessToken",
            "clientId",
            "expiresAt",
            "refreshToken",
            "resource",
            "tokenEndpoint",
        ];
        assert_eq!(members(&kept(&scratch, &remote)), six, "{listed}");
    }
}

#[test]
fn entries_after_the_first_sixteen_are_left_out_with_one_warning_and_nothing_is_asked_of_them() {
    let kinds: Vec<String> = (1..=17).map(|n| format!("k{n}")).collect();
    let scenario = kinds.iter().fold(
        RemoteScenario::accepting("doc-long-listing"),
        |listed, kind| listed.accompanied_by(Companion::new(kind, &[kind.as_str()])),
    );
    let (remote, scratch, output, visits) = login(scenario);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let mut asked = vec![remote.resource()];
    asked.extend(kinds[..16].iter().map(|kind| remote.companion(kind)));
    assert_eq!(every(&visits[0], "resource"), asked);
    let scope = format!("documents {}", kinds[..16].join(" "));
    assert_eq!(every(&visits[0], "scope"), [scope]);
    let targets = targets(&remote);
    assert!(targets.contains(&format!("{RESOURCE_METADATA}/k16")));
    let last = targets.iter().find(|target| target.ends_with("/k17"));
    assert_eq!(
        last, None,
        "nothing is asked of the seventeenth: {targets:?}"
    );
    let after = format!(
        "warning: the sign-in to {} leaves out every entry of `companion_resources` after the \
         first 16",
        remote.address()
    );
    assert_eq!(warnings(&output), [after]);
    let resources = &kept(&scratch, &remote)["resources"];
    assert_eq!(resources.as_array().map(Vec::len), Some(16));
}

#[test]
fn a_remote_that_lists_no_companion_resource_is_signed_in_to_and_kept_as_before() {
    for scenario in [
        RemoteScenario::accepting("doc-alone"),
        RemoteScenario::accepting("doc-alone-elsewhere")
            .accompanied_by(Companion::new("notes", &["notes"]).elsewhere()),
    ] {
        let (remote, scratch, output, visits) = login(scenario);
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        assert_eq!(every(&visits[0], "resource"), [remote.resource()]);
        assert_eq!(every(&visits[0], "scope"), ["documents"]);
        let six = [
            "accessToken",
            "clientId",
            "expiresAt",
            "refreshToken",
            "resource",
            "tokenEndpoint",
        ];
        assert_eq!(
            members(&kept(&scratch, &remote)),
            six,
            "and no other member"
        );
        let targets = targets(&remote);
        let read = targets.iter().filter(|t| t.starts_with(RESOURCE_METADATA));
        assert_eq!(
            read.count(),
            1,
            "the remote's own metadata alone: {targets:?}"
        );
        assert!(remote.requests_elsewhere().is_empty());
    }
}

#[test]
fn interface_login_signs_in_once_over_the_companion_resources_of_another_document_s_remote() {
    let remote = Remote::start(
        RemoteScenario::accepting(OTHER).accompanied_by(Companion::new("notes", &["notes"])),
    );
    let scratch = Scratch::with_reading_list();
    listing(&scratch, &remote, true);
    let args = ["interface", "login", HANDLE];
    let (output, browser) = signed_in::run(&scratch, &args, BrowserScenario::Visiting, None);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let visits = browser.visits();
    assert_eq!(visits.len(), 1, "one consent: {visits:?}");
    let notes = remote.companion("notes");
    assert_eq!(
        every(&visits[0], "resource"),
        [remote.resource(), notes.clone()]
    );
    assert_eq!(every(&visits[0], "scope"), ["documents notes"]);
    assert!(warnings(&output).is_empty(), "{output:?}");
    // The sign-in is kept under the address that the entry of `documents` names, over the
    // companion resource too.
    let sign_in = kept(&scratch, &remote);
    assert_eq!(sign_in["resource"], remote.resource());
    assert_eq!(sign_in["issuer"], remote.issuer());
    assert_eq!(
        sign_in["resources"],
        json!([{ "resource": notes, "kind": "notes" }])
    );
    assert!(remote.admits(sign_in["accessToken"].as_str().unwrap()));
}

#[test]
fn a_companion_resource_whose_metadata_cannot_be_reached_is_left_out_with_every_one_after_it() {
    let (remote, scratch, output, visits) = login(
        RemoteScenario::accepting("doc-unreached")
            .accompanied_by(Companion::new("notes", &["notes"]))
            .accompanied_by(Companion::new("gone", &["gone"]).faulty(CompanionFault::Unreadable))
            .accompanied_by(Companion::new("inbox", &["inbox"]).faulty(CompanionFault::Unanswered))
            .accompanied_by(Companion::new("mail", &["mail"])),
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(visits.len(), 1, "one consent: {visits:?}");
    let [notes, gone, inbox, mail] =
        ["notes", "gone", "inbox", "mail"].map(|kind| remote.companion(kind));
    assert_eq!(
        every(&visits[0], "resource"),
        [remote.resource(), notes.clone()]
    );
    assert_eq!(every(&visits[0], "scope"), ["documents notes"]);
    // The metadata answered `404` does not stop the next entry, whose metadata is not answered;
    // no entry after that one is asked about.
    let targets = targets(&remote);
    let asked = [&notes, &gone, &inbox, &mail].map(|resource| {
        let path = resource.strip_prefix(remote.origin()).unwrap();
        targets.contains(&format!("{RESOURCE_METADATA}{path}"))
    });
    assert_eq!(asked, [true, true, true, false], "{targets:?}");
    let warned = warnings(&output);
    assert_eq!(warned.len(), 2, "{warned:?}");
    assert!(
        warned[0].starts_with(&left_out(&remote, &gone, "gone", "")),
        "{warned:?}"
    );
    let unreached = warned[1].as_str();
    for said in [
        remote.address().as_str(),
        inbox.as_str(),
        "(inbox)",
        "its metadata cannot be reached",
        "each companion resource listed after it",
    ] {
        assert!(unreached.contains(said), "{said}: {unreached}");
    }
    assert!(!unreached.contains(&mail), "{unreached}");
    let resources = &kept(&scratch, &remote)["resources"];
    assert_eq!(resources, &json!([{ "resource": notes, "kind": "notes" }]));
}
