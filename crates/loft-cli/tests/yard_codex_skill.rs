//! The Codex plugin carries one composed Yard workflow and only native command hooks.

use std::fs;

use serde_json::Value;

use crate::common::workspace;

const SOURCE: &str = "crates/loft-cli/skill/yard";
const PACKAGE: &str = "plugins/codex/loft/skills/yard";

fn read(path: &str) -> String {
    fs::read_to_string(workspace().join(path)).unwrap()
}

fn adaptation() -> String {
    read(&format!("{SOURCE}/agents/codex.md"))
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn the_codex_yard_package_equals_the_shared_source_with_only_its_codex_adaptation() {
    let shared = read(&format!("{SOURCE}/SKILL.md"));
    let adapted = read(&format!("{SOURCE}/agents/codex.md"));
    let package = read(&format!("{PACKAGE}/SKILL.md"));
    assert_eq!(package, format!("{shared}\n{adapted}"));
    assert!(package.lines().count() <= 500);
    assert!(!adapted.starts_with("---"));
    assert!(!package.contains("Claude"));
    assert!(!package.contains("mcp__"));
    let frontmatter = package
        .strip_prefix("---\n")
        .unwrap()
        .split_once("\n---\n")
        .unwrap()
        .0;
    let fields: Vec<_> = frontmatter
        .lines()
        .map(|line| line.split_once(':').unwrap().0)
        .collect();
    assert_eq!(fields, ["name", "description"]);
    assert!(frontmatter.starts_with("name: yard\n"));
}

#[test]
fn the_codex_yard_adaptation_teaches_cli_identity_revisions_tasks_and_page_submissions() {
    let text = adaptation();
    for operation in ["read", "write", "wait", "withdraw", "close", "published"] {
        assert!(
            text.contains(&format!("loft yard {operation}")),
            "{operation}"
        );
    }
    for habit in [
        "--host codex",
        "--session",
        "--agent",
        "--agent-type",
        "--format json",
        "--mark-read",
        "--based-on",
        "--send",
        "--submission",
        "540",
        "pending",
        "update_plan",
        "active stage",
        "fix",
        "Yard page",
        "unavailable",
    ] {
        assert!(text.contains(habit), "{habit}");
    }
    assert!(
        text.contains("When Codex does not offer `update_plan`, write Yard task blocks yourself")
    );
}

#[test]
fn the_codex_yard_adaptation_keeps_page_approval_separate_from_native_controls() {
    let text = adaptation();
    for phrase in [
        "does not switch Codex out of plan mode",
        "permissions",
        "native questions",
        "never write the person's answers",
        "only the steps",
        "--reports",
    ] {
        assert!(text.contains(phrase), "{phrase}");
    }
}

#[test]
fn codex_plugin_hooks_use_the_native_adapter_with_bounded_commands_and_one_background_worker() {
    let plugin: Value = serde_json::from_str(&read("plugins/codex/loft/plugin.json")).unwrap();
    assert_eq!(plugin["hooks"], "./hooks/hooks.json");
    let manifest: Value =
        serde_json::from_str(&read("plugins/codex/loft/hooks/hooks.json")).unwrap();
    let hooks = manifest["hooks"].as_object().unwrap();
    let mut names: Vec<_> = hooks.keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "Interrupt",
            "PostCompact",
            "PostToolUse",
            "PreToolUse",
            "SessionEnd",
            "SessionStart",
            "Stop",
            "SubagentStart",
            "SubagentStop",
            "UserPromptSubmit"
        ]
    );
    for (name, groups) in hooks {
        let handlers = groups[0]["hooks"].as_array().unwrap();
        assert_eq!(handlers[0]["type"], "command");
        assert_eq!(handlers[0]["command"], "loft yard codex hook");
        let short = matches!(name.as_str(), "Interrupt" | "SessionEnd");
        assert_eq!(handlers[0]["timeout"], if short { 3 } else { 10 });
        assert_eq!(handlers.len(), if short { 1 } else { 2 });
        if !short {
            assert_eq!(handlers[1]["command"], "loft yard codex watch");
            assert_eq!(handlers[1]["type"], "command");
            assert_eq!(handlers[1]["async"], true);
            assert_eq!(handlers[1]["timeout"], 86400);
        }
    }
}
