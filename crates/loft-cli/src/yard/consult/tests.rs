//! The shared cases, which the Claude mod's `consult.ts` is held to as well, and the patch
//! headers of a Codex edit.

use serde_json::Value;

use super::{Class, class_of, loft_subcommands, patched, shell_reads};

/// The cases in `plugins/claude/loft/tests/consult-cases.ts`, which keeps them as JSON between
/// two markers so that both suites read one table.
fn cases() -> Value {
    let text = include_str!("../../../../../plugins/claude/loft/tests/consult-cases.ts");
    let start = text.find("/* consult-cases:start */").unwrap() + "/* consult-cases:start */".len();
    let end = text.find("/* consult-cases:end */").unwrap();
    serde_json::from_str(&text[start..end]).unwrap()
}

fn trees(cases: &Value) -> Vec<String> {
    cases["trees"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tree| tree.as_str().unwrap().to_owned())
        .collect()
}

fn names(classes: &[Class]) -> Vec<&'static str> {
    classes.iter().map(|class| class.name()).collect()
}

#[test]
fn each_path_of_the_shared_cases_takes_its_class() {
    let cases = cases();
    let trees = trees(&cases);
    for case in cases["paths"].as_array().unwrap() {
        let (path, dir) = (
            case["path"].as_str().unwrap(),
            case["dir"].as_str().unwrap(),
        );
        assert_eq!(
            class_of(path, dir, &trees).name(),
            case["class"],
            "{path} from {dir}"
        );
    }
}

#[test]
fn each_shell_command_of_the_shared_cases_reads_its_classes_and_names_its_loft_subcommands() {
    let cases = cases();
    let trees = trees(&cases);
    for case in cases["shell"].as_array().unwrap() {
        let (command, dir) = (
            case["command"].as_str().unwrap(),
            case["dir"].as_str().unwrap(),
        );
        let reads = names(&shell_reads(command, dir, &trees));
        assert_eq!(
            serde_json::json!(reads),
            case["reads"],
            "reads of {command:?}"
        );
        assert_eq!(
            serde_json::json!(loft_subcommands(command)),
            case["loft"],
            "loft of {command:?}"
        );
    }
}

#[test]
fn a_patch_edits_each_file_it_adds_updates_deletes_or_moves_to() {
    let patch = "*** Begin Patch\n\
                 *** Add File: spec/new.yaml\n\
                 +name: x\n\
                 *** Update File: src/a.rs\n\
                 *** Move to: src/b.rs\n\
                 @@\n\
                 -old\n\
                 +new\n\
                 *** Delete File: notes.md\n\
                 *** End Patch\n";
    assert_eq!(
        patched(patch),
        ["spec/new.yaml", "src/a.rs", "src/b.rs", "notes.md"]
    );
    assert_eq!(
        patched("*** Begin Patch\n*** End Patch\n"),
        Vec::<String>::new()
    );
}
