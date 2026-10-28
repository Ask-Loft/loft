//! The Codex plugin carries one composed Yard workflow and only native command hooks.

use std::fs;

use serde_json::Value;

use crate::common::workspace;
use crate::plugins::drifted;

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
    assert_eq!(
        drifted(PACKAGE),
        Vec::<String>::new(),
        "run scripts/package-skill.sh"
    );
    assert!(package.starts_with(&shared) && package.ends_with(&adapted));
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
fn codex_usage_guidance_distinguishes_app_server_observation_from_native_hook_coverage() {
    let text = adaptation();
    for phrase in [
        "Native hooks carry no token counters",
        "already running local Codex app server",
        "--app-server-socket ABSOLUTE_PATH",
        "500 milliseconds",
        "unavailable or partial",
        "reconnecting imports no historical token totals",
        "absent cache-write counter stays unavailable",
        "Native activity and heartbeats continue",
    ] {
        assert!(text.contains(phrase), "{phrase}");
    }
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
fn codex_planning_retains_its_versioned_upstream_source_and_attribution() {
    let text = adaptation();
    assert!(text.contains(
        "https://github.com/openai/codex/blob/rust-v0.164.0/codex-rs/collaboration-mode-templates/templates/plan.md"
    ));
    assert!(text.contains("OpenAI Codex"));
    assert!(text.contains("Apache-2.0"));
    let notice = read("NOTICE");
    assert!(notice.contains("OpenAI Codex"));
    assert!(notice.contains("Copyright 2025 OpenAI"));
    assert!(notice.contains("planning workflow"));
}

#[test]
fn codex_planning_explores_then_clarifies_intent_and_implementation() {
    let text = adaptation();
    let phases = [
        "#### 1. Explore the repository",
        "#### 2. Clarify intent",
        "#### 3. Resolve implementation",
    ];
    let positions: Vec<_> = phases
        .iter()
        .map(|phase| text.find(phase).unwrap_or_else(|| panic!("no {phase}")))
        .collect();
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
    for phrase in [
        "when the person asks for a plan",
        "after specification approval",
        "Explore before asking about discoverable facts",
        "goal and success criteria",
        "scope, constraints and material tradeoffs",
        "interfaces and data flow",
        "failure cases",
        "compatibility and migrations",
        "verification and acceptance criteria",
    ] {
        assert!(text.contains(phrase), "{phrase}");
    }
}

#[test]
fn codex_planning_defers_product_mutation_and_keeps_native_controls_authoritative() {
    let text = adaptation();
    for phrase in [
        "Do not implement the change while planning",
        "do not edit product files, specifications or tests",
        "Yard planning records are the only writes this workflow permits",
        "Native mode and permissions still govern those writes",
        "`update_plan` is an execution-progress checklist",
        "does not enter or exit native Plan mode",
        "If native Plan mode prevents a Yard write, keep the plan in the conversation",
        "No prompt in this skill changes Codex's native collaboration mode",
    ] {
        assert!(text.contains(phrase), "{phrase}");
    }
    for native_declaration in [
        "You are in **Plan Mode** until a developer message explicitly ends it",
        "You are now in Plan mode",
        "## Mode rules (strict)",
    ] {
        assert!(!text.contains(native_declaration), "{native_declaration}");
    }
}

#[test]
fn codex_planning_uses_yard_questions_and_complete_plan_submissions_before_building() {
    let text = adaptation();
    for phrase in [
        "Required questions go through Yard submissions",
        "Do not replace an unanswered required decision with an assumed default",
        "one complete `plan` block with `title` and `markdown`",
        "summary, implementation steps, interfaces, tests and assumptions",
        "wait for approval before implementation",
        "Revise a rejected plan as a complete replacement",
        "new submission",
        "Do not put `<proposed_plan>` tags in the Yard plan's Markdown",
        "No question answer, timeout or native plan rendering counts as Yard plan approval",
        "An approved Yard plan authorizes only its described build when native mode and permissions allow it",
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

#[test]
fn codex_decision_guidance_uses_automatic_monitoring_with_explicit_wait_fallback() {
    let text = adaptation();
    for phrase in [
        "When decision notifications are available",
        "end your turn after sending the submission",
        "worker monitors fully resolved submissions",
        "automatic monitoring fulfills the shared workflow's wait",
        "unavailable, uncertain or not reported",
        "wait with `--submission`",
        "Never treat a timeout as approval",
        "read with `--mark-read` using your supplied caller identity",
        "exceptional duplicate notification",
        "fresh Yard read",
        "not a new approval",
        "Interruption or session end disarms automatic continuation",
        "genuine user prompt can rearm",
        "Normal turn completion leaves monitoring armed",
    ] {
        assert!(text.contains(phrase), "{phrase}");
    }
}

#[test]
fn codex_cache_write_guidance_lies_in_the_skill_and_not_in_the_session_context() {
    let text = adaptation();
    let yard = crate::yard::Yard::new();
    let reply = crate::yard_codex::hook(&yard, &crate::yard_codex::event(&yard, "SessionStart"));
    let context = reply["hookOutput"]["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    for phrase in [
        "Zero cache writes from ChatGPT or unclassified sources are unavailable",
        "Verified API-key zeros and positive reported counts remain measured",
        "Raw source values are preserved",
    ] {
        assert!(text.contains(phrase), "{phrase}");
        assert!(!context.contains(phrase), "{phrase}");
    }
    assert!(!context.contains("usage"), "{context}");
}
