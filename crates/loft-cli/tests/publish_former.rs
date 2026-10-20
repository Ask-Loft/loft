//! `loft publish` over a tree in which a decision names, under `formerLabel`, a label the
//! publish would give back (Label allocation at publish): the publish is refused and writes
//! nothing, and once `loft fmt` names the node by its key again the tree is published and
//! checks clean.

use std::fs;
use std::process::Output;

use crate::common::{EPOCH, Scratch, stderr};
use crate::schema;
use crate::snapshot::{changed, snapshot};
use serde_json::Value;
use software_plant::{Clock, GitScenario};

const SUPERSEDED: &str = "decisions/store-a-shelf-as-a-directory.yaml";
const SUPERSEDED_KEY: &str = "00000000-0000-4000-8000-00000000006e";
const STORE: &str = "components/shelf-store.yaml";
const CONSTRAINT: &str = "00000000-0000-4000-8000-00000000003e";

fn on_main(scratch: &Scratch, args: &[&str]) -> Output {
    scratch.loft_with(
        args,
        Some(&GitScenario::clean_on("main")),
        &Clock::fixed(EPOCH),
    )
}

fn json(output: &Output) -> Value {
    schema::conforming(
        &output.stdout,
        include_str!("../../../schemas/publish.json"),
    )
}

/// The Reading List with its superseded decision naming the Constraint `CON-2`, published once
/// after the Constraint was deleted and formatted away, then the Constraint restored as it was
/// and not formatted: the decision names `CON-2` under `formerLabel`.
fn restored_unformatted() -> Scratch {
    let scratch = Scratch::with_reading_list();
    scratch.edit(
        SUPERSEDED,
        "subjects:\n",
        &format!("subjects:\n  - role: affected\n    node: {CONSTRAINT}\n"),
    );
    let store = fs::read_to_string(scratch.file(STORE)).unwrap();
    let start = store.find("constraints:\n").unwrap();
    let end = store.find("qualityAttributes:\n").unwrap();
    let without = format!("{}{}", &store[..start], &store[end..]);
    fs::write(scratch.file(STORE), without).unwrap();
    assert_eq!(scratch.loft(&["fmt"]).status.code(), Some(0));
    let decision = fs::read_to_string(scratch.file(SUPERSEDED)).unwrap();
    assert!(decision.contains("formerLabel: CON-2\n"), "{decision}");
    let published = on_main(&scratch, &["publish"]);
    assert_eq!(published.status.code(), Some(0), "{}", stderr(&published));
    fs::write(scratch.file(STORE), store).unwrap();
    scratch
}

#[test]
fn a_publish_that_would_give_back_a_label_a_decision_names_as_former_is_refused_writing_nothing() {
    let scratch = restored_unformatted();
    let before = snapshot(&scratch.0);
    let output = on_main(&scratch, &["publish", "--format", "json"]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let document = json(&output);
    assert_eq!(document["outcome"], "error");
    let message = document["errors"][0]["message"].as_str().unwrap();
    for said in [
        "CON-2",
        SUPERSEDED_KEY,
        "formatting writes each such reference as the node's key",
        "loft fmt",
    ] {
        assert!(message.contains(said), "{said}: {message}");
    }
    assert_eq!(
        changed(&before, &snapshot(&scratch.0)),
        Vec::<String>::new()
    );
}

#[test]
fn the_tree_formatted_after_that_refusal_is_published_and_checks_clean() {
    let scratch = restored_unformatted();
    assert_eq!(scratch.loft(&["fmt"]).status.code(), Some(0));
    let decision = fs::read_to_string(scratch.file(SUPERSEDED)).unwrap();
    assert!(!decision.contains("formerLabel"), "{decision}");
    let output = on_main(&scratch, &["publish"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let check = scratch.loft(&["check"]);
    assert_eq!(check.status.code(), Some(0), "{}", stderr(&check));
}
