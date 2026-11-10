//! What a Codex tool call consults, on its `tool.start`: the `loft` subcommands and the classes
//! of what a shell command reads, and the classes of the files an `apply_patch` edits (the
//! Features "Classify what a tool call consults" and "Codex Yard hook"). Codex runs
//! `apply_patch` as a tool of its own or, as Codex 0.164 does, as a shell command whose heredoc
//! holds the patch. Only the names and the classes leave here, never the command or the patch.

use std::path::Path;

use serde_json::{Value, json};
use yardlog_store::git::YardGit;
use yardlog_store::home::Home;

use super::translate::Hook;
use crate::yard::consult::{
    Class, class_of, loft_subcommands, ordered, patched, runs, shell_reads,
};
use crate::yard::serve::trees;

/// The trees a path may lie in: those the page server serves, and the worktree that holds the
/// hook's directory where none of those holds it.
pub fn known_trees(dir: &str) -> Vec<String> {
    let mut known: Vec<String> = Home::from_env()
        .and_then(|home| trees::served(&home).ok())
        .unwrap_or_default()
        .into_iter()
        .map(|tree| tree.display().to_string())
        .collect();
    let held = known
        .iter()
        .any(|tree| class_of(dir, "/", std::slice::from_ref(tree)) != Class::Other);
    if !held && let Ok(top) = YardGit::default().top_level(Path::new(dir)) {
        known.push(top.display().to_string());
    }
    known
}

/// The text of the tool's input that holds its command or its patch.
fn input_text(input: &Value) -> Option<&str> {
    input.as_str().or_else(|| {
        ["command", "input", "patch"]
            .iter()
            .find_map(|field| input[field].as_str())
    })
}

fn names(classes: &[Class]) -> Value {
    json!(classes.iter().map(|class| class.name()).collect::<Vec<_>>())
}

/// Adds `edits`, the classes of the files the patch in `text` edits, where it edits any.
fn insert_edits(
    fields: &mut serde_json::Map<String, Value>,
    text: &str,
    dir: &str,
    trees: &[String],
) {
    let edits = ordered(patched(text).iter().map(|path| class_of(path, dir, trees)));
    if !edits.is_empty() {
        fields.insert("edits".into(), names(&edits));
    }
}

/// Adds to the `tool.start` events of `hook` what its call consults, its paths classed among
/// `trees`.
pub fn add(events: &mut [Value], hook: &Hook, trees: &[String]) {
    let dir = hook.0["cwd"].as_str().unwrap_or_default();
    let tool = hook.0["tool_name"].as_str().unwrap_or_default();
    let Some(text) = input_text(&hook.0["tool_input"]) else {
        return;
    };
    let mut fields = serde_json::Map::new();
    match tool {
        "Bash" => {
            let loft = loft_subcommands(text);
            if !loft.is_empty() {
                fields.insert("loft".into(), json!(loft));
            }
            let reads = shell_reads(text, dir, trees);
            if !reads.is_empty() {
                fields.insert("reads".into(), names(&reads));
            }
            if runs(text, "apply_patch") {
                insert_edits(&mut fields, text, dir, trees);
            }
        }
        "apply_patch" => insert_edits(&mut fields, text, dir, trees),
        _ => {}
    }
    for event in events
        .iter_mut()
        .filter(|event| event["kind"] == "tool.start")
    {
        for (name, value) in &fields {
            event[name] = value.clone();
        }
    }
}
