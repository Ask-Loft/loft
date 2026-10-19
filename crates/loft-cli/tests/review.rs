//! `loft review` as a process (the `loft review` Feature): the packet of the Reading List's
//! assertions, against a base revision's tree read from git, narrowed to a node and paged by
//! whole units, and what it refuses.

use std::fs;
use std::process::Output;

use crate::base::{COMMIT, based_on_now};
use crate::common::{EPOCH, Scratch, stderr, stdout};
use crate::schema;
use crate::snapshot::snapshot;
use serde_json::Value;
use software_plant::{Clock, GitScenario};

const FEATURE: &str = "products/reading-list/shelf/remove-a-book.yaml";
const PLAIN: &str = "\"A removed book no longer lists.\"";
const EDITED: &str = "\"A removed book no longer shows.\"";
/// The criterion AC-5 of the Feature "Remove a book", whose name `EDITED` changes.
const GONE: &str = "00000000-0000-4000-8000-000000000031";

fn review(scratch: &Scratch, args: &[&str], git: Option<&GitScenario>) -> Output {
    let mut all = vec!["review"];
    all.extend_from_slice(args);
    scratch.loft_with(&all, git, &Clock::fixed(EPOCH))
}

fn json(output: &Output) -> Value {
    schema::conforming(&output.stdout, include_str!("../../../schemas/review.json"))
}

fn packet(scratch: &Scratch, args: &[&str], git: Option<&GitScenario>) -> Value {
    let mut all = args.to_vec();
    all.extend(["--format", "json"]);
    let output = review(scratch, &all, git);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    json(&output)
}

fn units(document: &Value) -> &Vec<Value> {
    document["units"].as_array().unwrap()
}

/// The key of each unit's scope.
fn scopes(document: &Value) -> Vec<String> {
    units(document)
        .iter()
        .map(|unit| unit["scope"]["node"]["key"].as_str().unwrap().to_owned())
        .collect()
}

/// The Reading List, its base the tree as it is, then one criterion renamed.
fn edited() -> (Scratch, GitScenario) {
    let scratch = Scratch::with_reading_list();
    let git = based_on_now(&scratch);
    scratch.edit(FEATURE, PLAIN, EDITED);
    (scratch, git)
}

#[test]
fn the_packet_names_its_provenance_and_its_page() {
    let scratch = Scratch::with_reading_list();
    let document = packet(&scratch, &[], None);
    assert_eq!(document["outcome"], "packed");
    let provenance = &document["provenance"];
    assert_eq!(provenance["loft"], env!("CARGO_PKG_VERSION"));
    let root = provenance["stateRoot"].as_str().unwrap();
    assert!(
        root.len() == 64 && root.bytes().all(|b| b.is_ascii_hexdigit()),
        "{root}"
    );
    assert_eq!(provenance["base"], Value::Null);
    assert_eq!(provenance["node"], Value::Null);
    assert_eq!(
        document["page"],
        serde_json::json!({ "number": 1, "of": 1, "maxChars": 50000 })
    );
}

#[test]
fn without_a_base_every_assertion_is_reviewed_with_no_change() {
    let scratch = Scratch::with_reading_list();
    let document = packet(&scratch, &[], None);
    let assertions: Vec<&Value> = units(&document)
        .iter()
        .flat_map(|unit| unit["assertions"].as_array().unwrap())
        .collect();
    assert!(assertions.iter().any(|assertion| assertion["key"] == GONE));
    assert!(
        assertions
            .iter()
            .all(|assertion| assertion["change"].is_null())
    );
}

#[test]
fn each_node_carries_its_key_label_kind_site_name_and_view() {
    let scratch = Scratch::with_reading_list();
    let document = packet(&scratch, &["--node", "remove-a-book"], None);
    let criterion = units(&document)[0]["requirements"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|each| each["criteria"].as_array().unwrap())
        .find(|criterion| criterion["key"] == GONE)
        .unwrap()
        .clone();
    assert_eq!(criterion["label"], "AC-5");
    assert_eq!(criterion["kind"], "criterion");
    assert_eq!(criterion["path"], format!("spec/{FEATURE}"));
    assert!(criterion["line"].as_u64().unwrap() > 1 && criterion["column"].as_u64().unwrap() > 1);
    assert_eq!(criterion["name"], "A removed book no longer lists.");
    assert!(criterion["view"].is_array());
}

#[test]
fn against_a_base_a_changed_assertion_is_reviewed_beside_the_base_s_node() {
    let (scratch, git) = edited();
    let document = packet(&scratch, &["--base", "develop"], Some(&git));
    assert_eq!(units(&document).len(), 1);
    let assertions = units(&document)[0]["assertions"].as_array().unwrap();
    assert_eq!(assertions.len(), 1, "{assertions:?}");
    assert_eq!(assertions[0]["key"], GONE);
    assert_eq!(assertions[0]["change"], "changed");
    assert_eq!(
        assertions[0]["before"]["name"],
        "A removed book no longer lists."
    );
    let base = &document["provenance"]["base"];
    assert_eq!(base["revision"], "develop");
    assert_eq!(base["commit"], COMMIT);
}

#[test]
fn a_removed_assertion_carries_the_base_s_node_and_site() {
    let scratch = Scratch::with_reading_list();
    let git = based_on_now(&scratch);
    let gone = format!("      - key: {GONE}\n        name: {PLAIN}\n");
    scratch.edit(FEATURE, &gone, "");
    assert_eq!(scratch.loft(&["fmt"]).status.code(), Some(0));
    let document = packet(&scratch, &["--base", "develop"], Some(&git));
    let assertion = &units(&document)[0]["assertions"][0];
    assert_eq!(assertion["change"], "removed");
    assert_eq!(assertion["before"]["path"], format!("spec/{FEATURE}"));
}

#[test]
fn a_tree_the_same_as_its_base_gives_an_empty_packet() {
    let scratch = Scratch::with_reading_list();
    let git = based_on_now(&scratch);
    let document = packet(&scratch, &["--base", "develop"], Some(&git));
    assert_eq!(document["outcome"], "empty");
    assert_eq!(units(&document).len(), 0);
}

#[test]
fn two_runs_print_the_same_bytes_and_write_nothing() {
    let (scratch, git) = edited();
    let before = snapshot(&scratch.0);
    for args in [
        &["--base", "develop", "--format", "json"][..],
        &["--base", "develop"],
    ] {
        let first = review(&scratch, args, Some(&git));
        let second = review(&scratch, args, Some(&git));
        assert_eq!(first.status.code(), Some(0), "{}", stderr(&first));
        assert_eq!(first.stdout, second.stdout);
    }
    assert_eq!(snapshot(&scratch.0), before);
}

#[test]
fn a_node_is_named_by_its_key_its_slug_or_its_label_in_any_case() {
    let scratch = Scratch::with_reading_list();
    let by_slug = packet(&scratch, &["--node", "remove-a-book"], None);
    let scope = &units(&by_slug)[0]["scope"]["node"];
    let key = scope["key"].as_str().unwrap();
    let label = scope["label"].as_str().unwrap().to_lowercase();
    for query in [key, &label] {
        let document = packet(&scratch, &["--node", query], None);
        assert_eq!(scopes(&document), [key], "{query}");
        assert_eq!(document["provenance"]["node"], query);
    }
}

#[test]
fn against_a_base_a_node_narrows_the_review_to_what_changed_below_it() {
    let (scratch, git) = edited();
    let elsewhere = packet(
        &scratch,
        &["--base", "develop", "--node", "add-a-book"],
        Some(&git),
    );
    assert_eq!(units(&elsewhere).len(), 0);
    let here = packet(
        &scratch,
        &["--base", "develop", "--node", "remove-a-book"],
        Some(&git),
    );
    assert_eq!(units(&here).len(), 1);
}

/// `loft review` with `args` refused: its standard error and its JSON document's first error.
fn refused(scratch: &Scratch, args: &[&str], git: Option<&GitScenario>) -> (String, Value) {
    let output = review(scratch, args, git);
    assert_eq!(output.status.code(), Some(1), "{}", stdout(&output));
    assert_eq!(stdout(&output), "");
    let mut all = args.to_vec();
    all.extend(["--format", "json"]);
    let document = json(&review(scratch, &all, git));
    assert_eq!(document["outcome"], "error");
    (stderr(&output), document["errors"][0].clone())
}

#[test]
fn a_node_that_names_nothing_is_refused_naming_the_query() {
    let scratch = Scratch::with_reading_list();
    let (text, error) = refused(&scratch, &["--node", "no-such-node"], None);
    assert_eq!(error["kind"], "node");
    assert!(text.contains("no-such-node"), "{text}");
}

#[test]
fn a_node_that_names_two_nodes_is_refused_naming_each_key() {
    let scratch = Scratch::with_reading_list();
    let from = scratch.file("products/reading-list/shelf/add-a-book.yaml");
    let text = fs::read_to_string(&from).unwrap();
    fs::remove_file(&from).unwrap();
    fs::write(
        scratch.file("products/reading-list/shelf/rq-1.yaml"),
        text.replace("slug: add-a-book", "slug: rq-1"),
    )
    .unwrap();
    let (text, error) = refused(&scratch, &["--node", "rq-1"], None);
    assert_eq!(error["kind"], "node");
    assert!(
        text.contains("00000000-0000-4000-8000-00000000001e"),
        "{text}"
    );
    assert!(
        text.contains("00000000-0000-4000-8000-000000000028"),
        "{text}"
    );
}

/// The compact JSON size of each unit of an unpaged packet of the Reading List.
fn sizes(scratch: &Scratch) -> Vec<usize> {
    let document = packet(scratch, &["--max-chars", "10000000"], None);
    units(&document)
        .iter()
        .map(|unit| serde_json::to_string(unit).unwrap().chars().count())
        .collect()
}

#[test]
fn pages_take_whole_units_in_order_up_to_the_limit() {
    let scratch = Scratch::with_reading_list();
    let sizes = sizes(&scratch);
    assert!(sizes.len() >= 3, "{sizes:?}");
    let limit = (sizes[0] + sizes[1]).to_string();
    let first = packet(&scratch, &["--max-chars", &limit], None);
    assert_eq!(units(&first).len(), 2);
    let taken: usize = units(&first)
        .iter()
        .map(|unit| serde_json::to_string(unit).unwrap().chars().count())
        .sum();
    assert!(taken <= sizes[0] + sizes[1]);
    let second = packet(&scratch, &["--max-chars", &limit, "--page", "2"], None);
    assert_eq!(second["page"]["number"], 2);
    assert!(units(&second)[0].get("oversized").is_none());
}

#[test]
fn a_unit_longer_than_the_limit_stands_alone_whole_and_marked_oversized() {
    let scratch = Scratch::with_reading_list();
    let count = sizes(&scratch).len();
    let document = packet(&scratch, &["--max-chars", "1", "--page", "2"], None);
    assert_eq!(document["page"]["of"], count);
    assert_eq!(units(&document).len(), 1);
    assert_eq!(units(&document)[0]["oversized"], true);
    let unpaged = packet(&scratch, &["--max-chars", "10000000"], None);
    let mut whole = units(&unpaged)[1].clone();
    whole["oversized"] = Value::Bool(true);
    assert_eq!(units(&document)[0], whole);
}

#[test]
fn a_page_past_the_last_is_refused_naming_the_number_of_pages() {
    let scratch = Scratch::with_reading_list();
    let (text, error) = refused(&scratch, &["--page", "9"], None);
    assert_eq!(error["kind"], "page");
    assert!(text.contains("1 page"), "{text}");
}

#[test]
fn the_text_is_an_index_of_the_page() {
    let scratch = Scratch::with_reading_list();
    let document = packet(&scratch, &[], None);
    let output = review(&scratch, &[], None);
    let text = stdout(&output);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), units(&document).len() + 1, "{text}");
    let first = &units(&document)[0];
    let label = first["scope"]["node"]["label"].as_str().unwrap();
    let name = first["scope"]["node"]["name"].as_str().unwrap();
    let count = first["assertions"].as_array().unwrap().len();
    let size = serde_json::to_string(first).unwrap().chars().count();
    assert_eq!(
        lines[0],
        format!("{label}  {name}  {count} assertions  {size} characters")
    );
    assert_eq!(*lines.last().unwrap(), "page 1 of 1");
}

#[test]
fn a_node_without_a_key_is_refused_at_its_file_line_and_column() {
    let scratch = Scratch::with_reading_list();
    let unkeyed = format!("{PLAIN}\n      - name: \"A removed book stays removed.\"\n");
    scratch.edit(FEATURE, &format!("{PLAIN}\n"), &unkeyed);
    let (text, error) = refused(&scratch, &[], None);
    assert_eq!(error["kind"], "read");
    assert!(
        text.starts_with(&format!("spec/{FEATURE}:22:9: ")),
        "{text}"
    );
    assert!(text.contains("`key`"), "{text}");
}

#[test]
fn a_conflict_record_is_refused_naming_its_file_and_key() {
    let scratch = Scratch::with_reading_list();
    let text = fs::read_to_string(scratch.file(FEATURE)).unwrap();
    fs::write(
        scratch.file(FEATURE),
        format!("{text}    conflicts:\n      - field: body\n        ours: \"Why books go.\"\n"),
    )
    .unwrap();
    let (error, _) = refused(&scratch, &[], None);
    assert!(error.starts_with(&format!("spec/{FEATURE}")), "{error}");
    assert!(error.contains("conflict"), "{error}");
}

#[test]
fn outside_a_repository_a_base_is_refused() {
    let scratch = Scratch::with_reading_list();
    let outside = GitScenario::outside_repository();
    let (text, error) = refused(&scratch, &["--base", "develop"], Some(&outside));
    assert_eq!(error["kind"], "base");
    assert!(text.contains("not inside a git repository"), "{text}");
}

#[test]
fn a_base_that_names_no_commit_or_shares_no_history_is_refused_naming_it() {
    let scratch = Scratch::with_reading_list();
    let git = GitScenario::clean_on("feature").unrelated("orphan");
    for (revision, says) in [
        ("nosuch", "`nosuch` names no commit\n"),
        ("orphan", "`orphan` shares no history with HEAD\n"),
    ] {
        let (text, error) = refused(&scratch, &["--base", revision], Some(&git));
        assert_eq!(error["kind"], "base");
        assert_eq!(text, says);
    }
}

#[test]
fn a_base_tree_that_does_not_read_is_refused_naming_the_commit_and_the_place() {
    let scratch = Scratch::with_reading_list();
    let git = based_on_now(&scratch).file_at(
        "develop",
        "spec/products/reading-list/shelf/broken.yaml",
        "ffff000000000000000000000000000000000000",
        "name: [broken\n",
    );
    let (text, error) = refused(&scratch, &["--base", "develop"], Some(&git));
    assert_eq!(error["kind"], "base");
    assert!(
        text.starts_with(&format!(
            "the tree at {COMMIT} does not read: spec/products/reading-list/shelf/broken.yaml:1:"
        )),
        "{text}"
    );
}
