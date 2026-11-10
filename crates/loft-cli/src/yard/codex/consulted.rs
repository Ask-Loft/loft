//! What a Codex tool call consults, for its `tool.start`: the `loft` subcommands and the classes
//! of what a shell command reads, and the classes of the files an `apply_patch` edits (the
//! Features "Classify what a tool call consults" and "Codex Yard hook"). Codex runs
//! `apply_patch` as a tool of its own or, as Codex 0.164 does, as a shell command whose heredoc
//! holds the patch. Only the names and the classes leave here, never the command or the patch.

use std::path::Path;

use serde_json::{Map, Value, json};
use yardlog_store::git::YardGit;
use yardlog_store::home::Home;

use super::translate::{Hook, HookKind};
use crate::yard::consult::{
    Class, class_of, commands_of, loft_names, ordered, patched, read_paths, runs_in,
};
use crate::yard::serve::trees;

/// The trees a path may lie in: those the page server serves, and the worktree that holds the
/// hook's directory, which may lie within one of them, as a linked worktree may.
fn known_trees(dir: &str) -> Vec<String> {
    let mut known: Vec<String> = Home::from_env()
        .and_then(|home| trees::served(&home).ok())
        .unwrap_or_default()
        .into_iter()
        .map(|tree| tree.display().to_string())
        .collect();
    if let Ok(top) = YardGit::default().top_level(Path::new(dir)) {
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

/// The fields a `PreToolUse` hook's `tool.start` gains for what its call consults: each command
/// parsed once, and the known trees looked up only where there is a path to class. Worked out
/// before the session's lock is taken, since it may ask git for the worktree.
pub fn fields(hook: &Hook) -> Map<String, Value> {
    let mut fields = Map::new();
    if hook.kind() != HookKind::PreToolUse {
        return fields;
    }
    let Some(text) = input_text(&hook.0["tool_input"]) else {
        return fields;
    };
    // The trees are named as git names them, by their real paths, so the directory is too.
    let dir = Path::new(hook.dir())
        .canonicalize()
        .map_or_else(|_| hook.dir().to_owned(), |dir| dir.display().to_string());
    let dir = dir.as_str();
    let (loft, read, edited, patched_in) = match hook.0["tool_name"].as_str() {
        Some("Bash") => {
            let commands = commands_of(text);
            let (edited, patched_in) = match runs_in(&commands, dir, "apply_patch") {
                Some(at) => (patched(text), at),
                None => (Vec::new(), dir.to_owned()),
            };
            (
                loft_names(&commands),
                read_paths(&commands, dir),
                edited,
                patched_in,
            )
        }
        Some("apply_patch") => (Vec::new(), Vec::new(), patched(text), dir.to_owned()),
        _ => return fields,
    };
    let trees = if read.is_empty() && edited.is_empty() {
        Vec::new()
    } else {
        known_trees(dir)
    };
    let reads = ordered(read.iter().map(|(path, from)| class_of(path, from, &trees)));
    let edits = ordered(
        edited
            .iter()
            .map(|path| class_of(path, &patched_in, &trees)),
    );
    for (name, value, empty) in [
        ("loft", json!(loft), loft.is_empty()),
        ("reads", names(&reads), reads.is_empty()),
        ("edits", names(&edits), edits.is_empty()),
    ] {
        if !empty {
            fields.insert(name.to_owned(), value);
        }
    }
    fields
}
