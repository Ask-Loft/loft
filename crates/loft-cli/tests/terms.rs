//! `loft terms` as a process: the term report it prints, its JSON document, the exit code, and
//! that it writes nothing.

use std::fs;
use std::process::Command;

use crate::common::{EPOCH, Scratch, offline, stderr, stdout};
use crate::schema;
use crate::snapshot::snapshot;
use serde_json::Value;
use software_plant::Clock;

const PRODUCT: &str = "products/reading-list/product.yaml";

/// The Reading List with a Product body that uses `audit entry` twice and `audit row` once, and
/// a Requirement whose subject is a term's alias.
fn with_a_group() -> Scratch {
    let scratch = Scratch::with_reading_list();
    scratch.edit(
        PRODUCT,
        "body: \"",
        "body: \"Each audit entry is kept. An audit row is kept. One audit entry is shown. ",
    );
    fs::write(
        scratch.file("terms.yaml"),
        "- key: 01a14b50-0000-7000-8000-000000000001\n  name: \"reader\"\n  definition: \"The person \
         who keeps the list.\"\n  aliases:\n    - \"owner\"\n",
    )
    .unwrap();
    scratch
}

fn json(output: &std::process::Output) -> Value {
    schema::conforming(&output.stdout, include_str!("../../../schemas/terms.json"))
}

#[test]
fn the_report_prints_each_group_with_its_phrases_counts_and_nodes() {
    let scratch = with_a_group();
    let output = scratch.loft(&["terms"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(
        text.contains(
            "hint TD-001: `audit` is followed by head nouns of the noun group `record`.\n"
        ),
        "{text}"
    );
    assert!(text.contains("\n  audit entry 2 (P-1)\n"), "{text}");
    assert!(text.contains("\n  audit row 1 (P-1)\n"), "{text}");
}

#[test]
fn the_subjects_of_each_product_are_printed_under_its_label_and_name() {
    let output = with_a_group().loft(&["terms"]);
    let text = stdout(&output);
    let start = text
        .find("subjects of P-1 · Reading List\n")
        .expect("the Product's subjects");
    let lines: Vec<&str> = text[start..]
        .lines()
        .skip(1)
        .take_while(|l| l.starts_with("  "))
        .collect();
    assert!(!lines.is_empty(), "{text}");
    assert!(
        lines
            .iter()
            .all(|line| line.split(' ').any(|word| word.parse::<usize>().is_ok())),
        "{lines:?}"
    );
}

#[test]
fn the_output_ends_with_the_number_of_groups_of_each_rule() {
    let text = stdout(&with_a_group().loft(&["terms"]));
    assert!(
        text.ends_with("groups: TD-001 1, TD-002 0, TD-003 0\n"),
        "{text}"
    );
}

#[test]
fn the_command_exits_0_and_writes_nothing() {
    let scratch = with_a_group();
    let before = snapshot(&scratch.0);
    let output = scratch.loft(&["terms"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(snapshot(&scratch.0), before);
}

#[test]
fn a_lint_configuration_in_error_is_reported_and_no_group_is() {
    let scratch = with_a_group();
    fs::write(scratch.file("lint.yaml"), "rules:\n  - rule: TD-099\n").unwrap();
    let output = scratch.loft(&["terms"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("spec/lint.yaml:2:"),
        "{}",
        stderr(&output)
    );
    assert!(!stdout(&output).contains("TD-001"));
    let document = json(&scratch.loft(&["terms", "--format", "json"]));
    assert_eq!(document["outcome"], "error");
    assert_eq!(document["errors"][0]["kind"], "lint-config");
}

#[test]
fn the_json_document_lists_each_group_and_each_products_subjects() {
    let scratch = with_a_group();
    let document = json(&scratch.loft(&["terms", "--format", "json"]));
    assert_eq!(document["outcome"], "reported");
    let group = &document["groups"][0];
    assert_eq!(group["rule"], "TD-001");
    assert_eq!(group["severity"], "hint");
    assert!(group["message"].as_str().unwrap().contains("`record`"));
    let phrase = &group["phrases"][0];
    assert_eq!(phrase["text"], "audit entry");
    assert_eq!(phrase["count"], 2);
    assert_eq!(phrase["nodes"][0]["label"], "P-1");
    assert!(phrase["nodes"][0]["key"].is_string());
    let product = &document["products"][0];
    assert_eq!(product["label"], "P-1");
    assert!(product["key"].is_string());
    assert!(
        product["subjects"]
            .as_array()
            .is_some_and(|all| !all.is_empty())
    );
}

#[test]
fn a_subject_carries_its_mark_and_an_alias_its_term() {
    let scratch = with_a_group();
    scratch.edit(PRODUCT, "body: \"", "body: \"Kept. ");
    let feature = "products/reading-list/sharing/share-a-list.yaml";
    let text = fs::read_to_string(scratch.file(feature)).unwrap();
    let system = text
        .lines()
        .find_map(|line| line.trim().strip_prefix("system: "))
        .expect("a formed statement")
        .to_owned();
    scratch.edit(
        feature,
        &format!("system: {system}"),
        "system: \"The owner\"",
    );
    let document = json(&scratch.loft(&["terms", "--format", "json"]));
    let subjects: Vec<&Value> = document["products"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|product| product["subjects"].as_array().unwrap())
        .collect();
    let owner = subjects
        .iter()
        .find(|subject| subject["text"] == "owner")
        .unwrap();
    assert_eq!(owner["mark"], "alias");
    assert_eq!(owner["term"], "reader");
    assert!(
        subjects
            .iter()
            .filter(|subject| subject["mark"] != "alias")
            .all(|subject| subject["term"].is_null())
    );
}

fn reports(command: impl Fn() -> Command) -> (String, String) {
    let run = |args: &[&str]| stdout(&command().args(args).output().unwrap());
    (run(&["terms", "--format", "json"]), run(&["terms"]))
}

#[test]
fn the_report_depends_on_neither_the_locale_nor_the_environment() {
    let scratch = with_a_group();
    let under = |locale: &str| {
        reports(|| {
            let mut command = scratch.command(None, &Clock::fixed(EPOCH));
            for name in ["LC_ALL", "LANG", "LC_CTYPE", "LC_COLLATE", "LC_MESSAGES"] {
                command.env(name, locale);
            }
            command
        })
    };
    let plain = under("C");
    assert!(plain.1.contains("TD-001"), "{}", plain.1);
    for locale in ["tr_TR.UTF-8", "de_DE.UTF-8", "ja_JP.UTF-8"] {
        assert_eq!(under(locale), plain, "{locale}");
    }
    let bare = reports(|| {
        let mut command = scratch.command(None, &Clock::fixed(EPOCH));
        offline(command.env_clear());
        Clock::fixed(EPOCH).apply(&mut command);
        command
    });
    let busy = reports(|| {
        let mut command = scratch.command(None, &Clock::fixed(EPOCH));
        command
            .env("LOFT_LINT", "off")
            .env("TERM", "xterm-256color")
            .env("COLUMNS", "20")
            .env("TZ", "Pacific/Kiritimati");
        command
    });
    assert_eq!(busy, bare);
    assert_eq!(bare, plain);
}

#[test]
fn a_check_reports_no_finding_of_the_term_report() {
    let output = with_a_group().loft(&["check", "--format", "json"]);
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    let rules: Vec<&str> = document["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|finding| finding["rule"].as_str())
        .collect();
    assert!(
        rules.iter().all(|rule| !rule.starts_with("TD-")),
        "{rules:?}"
    );
}
