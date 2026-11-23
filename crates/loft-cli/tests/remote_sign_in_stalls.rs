//! A sign-in over companion resources whose metadata a stalled origin holds without an answer, run
//! against the remote simulator and the browser simulator: the sign-in gives up on that metadata
//! after the time limit of a call, asks about no companion resource listed after it, and goes on.

use std::time::{Duration, Instant};

use serde_json::json;
use software_plant::BrowserScenario;
use software_plant::remote::{Companion, CompanionFault, Remote, RemoteScenario};

use crate::common::Scratch;
use crate::common::oauth::RESOURCE_METADATA;
use crate::common::signed_in::{self, name_remote};
use crate::remote_sign_in_companions::{every, kept, targets, warnings};

#[test]
fn a_companion_resource_whose_metadata_is_held_past_the_time_limit_is_left_out_with_every_one_after_it()
 {
    let remote = Remote::start(
        RemoteScenario::accepting("doc-stalled")
            .accompanied_by(Companion::new("notes", &["notes"]))
            .accompanied_by(Companion::new("inbox", &["inbox"]).faulty(CompanionFault::Held))
            .accompanied_by(Companion::new("mail", &["mail"])),
    );
    let scratch = Scratch::with_reading_list();
    name_remote(&scratch, Some(&remote.address()), true);
    let (mut command, browser) = signed_in::command(&scratch, BrowserScenario::Visiting, None);
    let started = Instant::now();
    let output = command
        .env("LOFT_REMOTE_TIMEOUT", "1")
        .args(["remote", "login"])
        .output()
        .unwrap();
    let took = started.elapsed();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    // The held request is given up after the time limit of one call, not the default one.
    assert!(took < Duration::from_secs(20), "{took:?}");
    let [notes, inbox, mail] = ["notes", "inbox", "mail"].map(|kind| remote.companion(kind));
    let visits = browser.visits();
    assert_eq!(visits.len(), 1, "one consent: {visits:?}");
    assert_eq!(
        every(&visits[0], "resource"),
        [remote.resource(), notes.clone()]
    );
    assert_eq!(every(&visits[0], "scope"), ["documents notes"]);
    let targets = targets(&remote);
    let asked = [&notes, &inbox, &mail].map(|resource| {
        let path = resource.strip_prefix(remote.origin()).unwrap();
        targets.contains(&format!("{RESOURCE_METADATA}{path}"))
    });
    assert_eq!(asked, [true, true, false], "{targets:?}");
    let warned = warnings(&output);
    assert_eq!(warned.len(), 1, "{warned:?}");
    for said in [
        remote.address().as_str(),
        inbox.as_str(),
        "its metadata cannot be reached",
        "each companion resource listed after it",
    ] {
        assert!(warned[0].contains(said), "{said}: {warned:?}");
    }
    let resources = &kept(&scratch, &remote)["resources"];
    assert_eq!(resources, &json!([{ "resource": notes, "kind": "notes" }]));
}
