//! What a sign-in over a remote's companion resources says, as text that the remote or
//! `loft/remotes.json` gave: each warning for a companion resource left out, and each error of a
//! sign-in made for a former resource or of a refused repeat of a recorded refresh, writes each
//! control character and each bidirectional control escaped; and a listed companion resource
//! whose identifier holds a character outside visible ASCII is left out before anything is asked
//! of it. Each runs under the remote and browser simulators, or under a remote of the test's own
//! where no scenario of the simulator gives such text.

use serde_json::json;
use software_plant::http::Server;
use software_plant::remote::{Companion, CompanionFault, RESOURCE, RemoteScenario};

use crate::common::connected::connected;
use crate::common::oauth::RESOURCE_METADATA;
use crate::common::signed_in::{granted_over, keep, kept_from};
use crate::remote_escapes::{
    ESCAPED, HOSTILE, Spoiled, Spoiling, assert_named, assert_shown, login_at,
};
use crate::remote_sign_in_companions::{every, kept, targets, warnings};
use crate::remote_sign_in_refusals::login;

/// The warning that a sign-in to the remote at `address` leaves out `named`, and why.
fn left_out(address: &str, named: &str, why: &str) -> String {
    format!("warning: the sign-in to {address} leaves out {named}: {why}")
}

#[test]
fn a_sign_in_names_each_entry_it_leaves_out_escaped_and_asks_nothing_of_one_outside_visible_ascii()
{
    let (remote, scratch, output, visits) = login(
        RemoteScenario::accepting("doc-hostile-listing")
            .accompanied_by(Companion::new("notes", &["notes"]))
            .listing(json!([
                { "resource": "{origin}/notes", "kind": "notes" },
                { "resource": "{origin}/in\u{9b}box\u{202e}", "kind": HOSTILE },
                { "resource": format!("{{origin}}/part#{HOSTILE}"), "kind": "fragment" },
                { "resource": "{origin}/notes", "kind": format!("again{HOSTILE}") },
            ])),
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_shown(&output);
    let (address, origin, notes) = (remote.address(), remote.origin(), remote.companion("notes"));
    assert_eq!(
        warnings(&output),
        [
            left_out(
                &address,
                &format!("{origin}/in\\u{{9b}}box\\u{{202e}} ({ESCAPED})"),
                "its identifier holds a character other than visible ASCII"
            ),
            left_out(
                &address,
                &format!("{origin}/part#{ESCAPED} (fragment)"),
                "its identifier holds user information or a fragment"
            ),
            left_out(
                &address,
                &format!("{notes} (again{ESCAPED})"),
                "an entry before it names the same resource or kind"
            ),
        ]
    );
    // The remote's metadata and the companion resource's are each read once, and nothing of an
    // entry left out.
    let targets = targets(&remote);
    let read: Vec<&String> = (targets.iter())
        .filter(|target| target.starts_with(RESOURCE_METADATA))
        .collect();
    let own = format!("{RESOURCE_METADATA}{RESOURCE}");
    let companion = format!("{RESOURCE_METADATA}/notes");
    assert_eq!(read, [&own, &companion], "{targets:?}");
    assert_eq!(
        every(&visits[0], "resource"),
        [remote.resource(), notes.clone()]
    );
    assert_eq!(
        kept(&scratch, &remote)["resources"],
        json!([{ "resource": notes, "kind": "notes" }])
    );
}

#[test]
fn a_sign_in_names_a_companion_whose_metadata_does_not_vouch_for_it_or_cannot_be_reached_escaped() {
    let kind = |name: &str| format!("{name}{HOSTILE}");
    let (remote, scratch, output, visits) = login(
        RemoteScenario::accepting("doc-hostile-kinds")
            .accompanied_by(
                Companion::new(&kind("notes"), &["notes"])
                    .at("/notes")
                    .faulty(CompanionFault::Misnamed),
            )
            .accompanied_by(Companion::new("inbox", &["inbox", &kind("in")]))
            .accompanied_by(
                Companion::new(&kind("mail"), &["mail"])
                    .at("/mail")
                    .faulty(CompanionFault::Unanswered),
            ),
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_shown(&output);
    let address = remote.address();
    let [notes, inbox, mail] =
        [kind("notes"), "inbox".to_owned(), kind("mail")].map(|kind| remote.companion(&kind));
    let warned = warnings(&output);
    assert_eq!(warned.len(), 3, "{warned:?}");
    assert_eq!(
        warned[0],
        left_out(
            &address,
            &format!("{notes} (notes{ESCAPED})"),
            &format!("its metadata names another resource, {notes}/elsewhere")
        )
    );
    assert_eq!(
        warned[1],
        left_out(
            &address,
            &format!("{inbox} (inbox)"),
            &format!("its metadata lists \"in{ESCAPED}\", which is not a scope")
        )
    );
    let unreached = format!(
        "warning: the sign-in to {address} leaves out {mail} (mail{ESCAPED}) and each companion \
         resource listed after it: its metadata cannot be reached: {}{RESOURCE_METADATA}/mail: ",
        remote.origin()
    );
    assert!(warned[2].starts_with(&unreached), "{warned:?}");
    assert_eq!(every(&visits[0], "resource"), [remote.resource()]);
    let sign_in = kept(&scratch, &remote);
    assert!(sign_in.get("resources").is_none(), "{sign_in}");
}

#[test]
fn a_sign_in_names_the_resource_that_a_companion_s_metadata_names_in_its_place_escaped() {
    let server = Server::start(Spoiling(Spoiled::Companion));
    let origin = server.url();
    let (_scratch, output) = login_at(&format!("{origin}/doc"));
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert_shown(&output);
    let warned = left_out(
        &format!("{origin}/doc"),
        &format!("{origin}/notes (notes)"),
        &format!("its metadata names another resource, {origin}/notes#{ESCAPED}"),
    );
    assert_eq!(warnings(&output), [warned]);
}

#[test]
fn a_sign_in_made_for_a_former_resource_names_that_resource_escaped() {
    let run = connected(1, 1, RemoteScenario::accepting);
    let former = format!("{}/former#{HOSTILE}", run.remote.origin());
    let grant = run.remote.grant_over(&[&former]);
    let sign_in = kept_from(&run.remote, &former, &grant, 60);
    keep(&run.scratch, &json!({ run.remote.address(): sign_in }));
    run.change();
    let output = run.publish();
    let origin = run.remote.origin();
    assert_named(
        &output,
        &format!("a former resource of the remote, {origin}/former#{ESCAPED}"),
    );
    assert!(run.publish_requests().is_empty(), "nothing is published");
}

#[test]
fn a_recorded_refresh_the_remote_refuses_names_the_recorded_resource_escaped() {
    let run = connected(1, 1, |key| {
        RemoteScenario::accepting(key).accompanied_by(Companion::new("notes", &["notes"]))
    });
    let origin = run.remote.origin();
    let mut sign_in = granted_over(&run.remote, &["notes"], 3600);
    sign_in["refreshing"] = json!(format!("{origin}/retired#{HOSTILE}"));
    keep(&run.scratch, &json!({ run.remote.address(): sign_in }));
    run.change();
    let output = run.publish();
    assert_named(
        &output,
        &format!("recorded for {origin}/retired#{ESCAPED} was refused with `invalid_target`"),
    );
    assert!(run.publish_requests().is_empty(), "nothing is published");
}
