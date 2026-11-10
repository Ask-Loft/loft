//! What a Codex tool call consults, on its `tool.start`, through the shipped binary with
//! simulated git: the `loft` subcommands and the classes of a shell command's reads, the classes
//! of a patch's files, and the descriptor that declares them.

use serde_json::{Value, json};

use crate::yard::Yard;
use crate::yard_codex::{event, hook, state};

/// The `tool.start` event kept for a Codex `tool` call whose input is `input`.
fn started(yard: &Yard, call: &str, tool: &str, input: &Value) -> Value {
    let mut pre = event(yard, "PreToolUse");
    pre["tool_name"] = json!(tool);
    pre["tool_use_id"] = json!(call);
    pre["tool_input"] = input.clone();
    hook(yard, &pre);
    state(yard)["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|event| event["kind"] == "tool.start" && event["call"] == call)
        .cloned()
        .unwrap()
}

#[test]
fn a_codex_shell_command_names_its_loft_subcommands_and_the_classes_its_readers_read() {
    let yard = Yard::new();
    let start = started(
        &yard,
        "call-one",
        "Bash",
        &json!({"command":"loft find PRIVATE-LABEL && cat spec/PRIVATE-NAME.yaml && rg PRIVATE-WORD"}),
    );
    assert_eq!(start["loft"], json!(["find"]), "{start}");
    assert_eq!(start["reads"], json!(["spec", "code"]), "{start}");
    assert!(start.get("edits").is_none());
    let build = started(&yard, "call-two", "Bash", &json!({"command":"cargo test"}));
    assert!(
        build.get("reads").is_none() && build.get("loft").is_none(),
        "{build}"
    );
    assert!(!state(&yard).to_string().contains("PRIVATE"));
}

#[test]
fn a_codex_patch_names_the_classes_of_the_files_it_edits() {
    let yard = Yard::new();
    let patch = "*** Begin Patch\n*** Update File: spec/PRIVATE-A.yaml\n@@\n-a\n+b\n\
                 *** Add File: src/PRIVATE-B.rs\n+fn b() {}\n*** End Patch\n";
    for (call, input) in [
        ("as-text", json!(patch)),
        ("as-command", json!({"command":patch})),
    ] {
        let start = started(&yard, call, "apply_patch", &input);
        assert_eq!(start["edits"], json!(["spec", "code"]), "{start}");
        assert!(start.get("reads").is_none());
    }
    // Codex 0.164 runs a patch as a shell command whose heredoc holds it.
    let shell = format!("apply_patch <<'PATCH'\n{patch}PATCH\n");
    let start = started(&yard, "as-shell", "Bash", &json!({"command":shell}));
    assert_eq!(start["edits"], json!(["spec", "code"]), "{start}");
    assert!(start.get("reads").is_none(), "{start}");
    let mention = started(
        &yard,
        "mention",
        "Bash",
        &json!({"command":"echo apply_patch"}),
    );
    assert!(mention.get("edits").is_none(), "{mention}");
    assert!(!state(&yard).to_string().contains("PRIVATE"));
}

#[test]
fn a_codex_patch_run_after_cd_names_its_files_from_where_it_runs() {
    let yard = Yard::new();
    let patch = "*** Begin Patch\n*** Update File: spec/a.yaml\n@@\n-a\n+b\n*** End Patch\n";
    let shell = format!("cd crates && apply_patch <<'PATCH'\n{patch}PATCH\n");
    let start = started(&yard, "after-cd", "Bash", &json!({"command":shell}));
    assert_eq!(start["edits"], json!(["code"]), "{start}");
    let shell = format!("(cd crates) && apply_patch <<'PATCH'\n{patch}PATCH\n");
    let start = started(&yard, "after-subshell", "Bash", &json!({"command":shell}));
    assert_eq!(start["edits"], json!(["spec"]), "{start}");
}

#[cfg(unix)]
#[test]
fn a_codex_hook_from_a_linked_directory_classes_its_paths_in_the_tree_it_leads_to() {
    let yard = Yard::new();
    let link = yard.scratch.home().join("linked");
    std::os::unix::fs::symlink(yard.scratch.0.canonicalize().unwrap(), &link).unwrap();
    let mut pre = event(&yard, "PreToolUse");
    pre["cwd"] = json!(link);
    pre["tool_name"] = json!("Bash");
    pre["tool_use_id"] = json!("linked");
    pre["tool_input"] = json!({"command":"cat spec/a.yaml"});
    hook(&yard, &pre);
    let kept = state(&yard);
    let start = kept["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|event| event["call"] == "linked")
        .cloned()
        .unwrap();
    assert_eq!(start["reads"], json!(["spec"]), "{start}");
}

#[test]
fn the_codex_descriptor_declares_that_it_classifies_what_each_tool_call_consults() {
    let yard = Yard::new();
    hook(&yard, &event(&yard, "UserPromptSubmit"));
    let kept = state(&yard);
    assert_eq!(
        kept["events"][0]["adapter"]["capabilities"]["consults"],
        true
    );
}
