//! Yard Log of a format newer than this `loft` reads: read for what it can, with the records
//! whose kinds it knows and a count of those it left out, and never written.

use std::fs;

use crate::snapshot::snapshot;
use crate::yard::{ASK, BRANCH, Yard, block, document};

/// [`ASK`] written, then its files made over as a newer `loft` might have written them: a later
/// format, a field this `loft` does not know, and a block of a kind it does not know.
fn newer() -> Yard {
    let yard = Yard::new();
    let note = r#"[{"id": "n1", "kind": "note", "stage": "shape", "markdown": "Kept."}]"#;
    yard.write("s1", ASK, &[]);
    yard.write("s1", note, &[]);
    let folder = yard.folder(BRANCH);
    let root = folder.join("yardlog.yaml");
    let text = fs::read_to_string(&root).unwrap();
    fs::write(
        &root,
        text.replace("format: yardlog/1", "format: yardlog/2"),
    )
    .unwrap();
    for entry in fs::read_dir(&folder).unwrap() {
        let path = entry.unwrap().path();
        if path == root {
            continue;
        }
        let text = fs::read_to_string(&path).unwrap();
        let text = text.replace("kind: question", "kind: sketch");
        let text = text.replace(
            "    markdown: \"Kept.\"",
            "    markdown: \"Kept.\"\n    colour: \"green\"",
        );
        fs::write(&path, text).unwrap();
    }
    yard
}

#[test]
fn a_read_of_a_newer_format_gives_the_records_this_loft_knows_and_counts_those_it_left_out() {
    let yard = newer();
    let read = yard.read("s1", &[]);
    assert_eq!(read["yardLog"]["format"], "yardlog/2");
    assert_eq!(read["leftOut"], 1);
    assert_eq!(block(&read, "n1")["version"]["body"]["markdown"], "Kept.");
    let stage = &read["stages"][0];
    let ids: Vec<&str> = stage["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|block| block["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["n1"]);
}

#[test]
fn a_write_to_a_newer_format_is_refused_naming_both_formats_and_changes_no_file() {
    let yard = newer();
    let before = snapshot(&yard.scratch.0);
    let output = yard.run_with(&["write", "--session", "s1"], ASK);
    let refused = document("write", &output);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(refused["errors"][0]["kind"], "newer");
    let message = refused["errors"][0]["message"].as_str().unwrap();
    assert!(
        message.contains("yardlog/2") && message.contains("yardlog/1"),
        "{message}"
    );
    assert!(snapshot(&yard.scratch.0) == before, "a file changed");
}
