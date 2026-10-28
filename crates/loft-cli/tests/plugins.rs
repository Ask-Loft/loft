//! The plugins for Claude Code and Codex: each committed package equals the composition `loft`
//! writes for its host, each marketplace, `ask-loft`, lists the plugin `loft` at its host's plugin root, and
//! every manifest carries the version of `loft`; the Claude Code plugin also names the hooks
//! module and the mod's type contract, and declares its settings, its mod
//! names the version of `loft` and the yard protocol `loft` speaks, and it holds the `loft-spec`
//! and `yard` skills; CI tests the mod under a pinned Claude Code and the Yard page in a DOM
//! emulator.

use std::fs;
use std::process::Command;

use crate::common::{Scratch, isolate, stderr, workspace};
use crate::schema;
use crate::workflow::job;
use serde_json::{Value, json};

fn read(path: &str) -> Value {
    let text = fs::read_to_string(workspace().join(path)).unwrap_or_else(|_| panic!("no {path}"));
    serde_json::from_str(&text).unwrap()
}

const CLAUDE_ROOT: &str = "plugins/claude/loft";
const CODEX_ROOT: &str = "plugins/codex/loft";

#[test]
fn the_repository_codex_skill_is_a_relative_link_to_the_shared_source() {
    let root = workspace();
    let entry = root.join(".agents/skills/loft-spec");
    assert!(
        fs::symlink_metadata(&entry)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        fs::read_link(&entry).unwrap(),
        std::path::Path::new("../../crates/loft-cli/skill/loft-spec")
    );
    let source = root.join("crates/loft-cli/skill/loft-spec");
    assert_eq!(
        entry.canonicalize().unwrap(),
        source.canonicalize().unwrap()
    );
    for path in ["SKILL.md", "agents/openai.yaml", "references/the-loop.md"] {
        assert_eq!(
            fs::read(entry.join(path)).unwrap(),
            fs::read(source.join(path)).unwrap()
        );
    }
}

/// The JSON document of `loft skill package --check` run at the repository's root, which
/// compares every committed package with the composition `loft` writes and writes nothing.
fn package_check() -> Value {
    let home = Scratch::empty();
    let mut command = Command::new(env!("CARGO_BIN_EXE_loft"));
    isolate(&mut command, &home.home());
    let output = command
        .current_dir(workspace())
        .args(["skill", "package", "--check", "--format", "json"])
        .output()
        .unwrap();
    assert!(
        [Some(0), Some(1)].contains(&output.status.code()),
        "{}",
        stderr(&output)
    );
    schema::conforming(
        &output.stdout,
        include_str!("../../../schemas/skill-package.json"),
    )
}

#[test]
fn each_committed_package_equals_the_composition_for_its_host() {
    let check = package_check();
    assert_eq!(
        check["outcome"], "current",
        "{check}: run scripts/package-skill.sh"
    );
    assert_eq!(
        check["packages"],
        json!([
            "plugins/claude/loft/skills/loft-spec",
            "plugins/claude/loft/skills/yard",
            "plugins/codex/loft/skills/loft-spec",
            "plugins/codex/loft/skills/yard",
        ])
    );
}

#[test]
fn the_packaging_script_runs_loft_skill_package_and_composes_nothing_itself() {
    let script = fs::read_to_string(workspace().join("scripts/package-skill.sh")).unwrap();
    let commands: Vec<&str> = script
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    assert_eq!(
        commands,
        [
            "set -eu",
            "cd \"$(dirname \"$0\")/..\"",
            "exec cargo run -q -p loft-cli -- skill package",
        ]
    );
}

/// The skills a plugin's package holds, by their folder names.
fn skills(root: &str) -> Vec<String> {
    let mut found: Vec<String> = fs::read_dir(workspace().join(root).join("skills"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    found.sort();
    found
}

#[test]
fn both_plugins_hold_loft_spec_and_their_adapters_yard_skills() {
    assert_eq!(skills(CLAUDE_ROOT), ["loft-spec", "yard"]);
    assert_eq!(skills(CODEX_ROOT), ["loft-spec", "yard"]);
}

#[test]
fn each_manifest_carries_the_version_of_loft() {
    for manifest in [
        "plugins/claude/loft/.claude-plugin/plugin.json",
        "plugins/codex/loft/plugin.json",
    ] {
        assert_eq!(
            read(manifest)["version"],
            env!("CARGO_PKG_VERSION"),
            "{manifest}"
        );
    }
}

#[test]
fn the_claude_code_marketplace_lists_the_plugin_loft_at_its_root() {
    let marketplace = read(".claude-plugin/marketplace.json");
    assert_eq!(marketplace["name"], "ask-loft");
    assert!(marketplace["owner"]["name"].is_string());
    let [entry] = marketplace["plugins"].as_array().unwrap().as_slice() else {
        panic!("one plugin");
    };
    assert_eq!(entry["name"], "loft");
    assert_eq!(entry["source"], format!("./{CLAUDE_ROOT}"));
    let manifest = read(&format!("{CLAUDE_ROOT}/.claude-plugin/plugin.json"));
    assert_eq!(manifest["name"], entry["name"]);
    for field in ["description", "author", "repository", "license"] {
        assert!(!manifest[field].is_null(), "{field}");
    }
}

#[test]
fn the_codex_marketplace_lists_the_plugin_loft_at_its_root_with_policy_and_category() {
    let marketplace = read(".agents/plugins/marketplace.json");
    assert_eq!(marketplace["name"], "ask-loft");
    let [entry] = marketplace["plugins"].as_array().unwrap().as_slice() else {
        panic!("one plugin");
    };
    assert_eq!(entry["name"], "loft");
    assert_eq!(
        entry["source"],
        json!({ "source": "local", "path": format!("./{CODEX_ROOT}") })
    );
    for field in ["installation", "authentication"] {
        assert!(entry["policy"][field].is_string(), "policy.{field}");
    }
    assert!(entry["category"].is_string());
    let manifest = read(&format!("{CODEX_ROOT}/plugin.json"));
    assert!(manifest.get("$schema").is_none());
    assert_eq!(manifest["hooks"], "./hooks/hooks.json");
    assert_eq!(manifest["name"], entry["name"]);
    for field in ["description", "author"] {
        assert!(!manifest[field].is_null(), "{field}");
    }
}

#[test]
fn the_claude_code_plugins_hooks_json_names_its_hooks_module_which_lies_beside_it() {
    let hooks = read(&format!("{CLAUDE_ROOT}/hooks/hooks.json"));
    assert_eq!(hooks["modules"], json!(["./register.tsx"]));
    assert!(
        workspace()
            .join(CLAUDE_ROOT)
            .join("hooks/register.tsx")
            .is_file()
    );
}

#[test]
fn the_claude_code_manifest_names_the_mods_type_contract_which_lies_in_the_plugin() {
    let manifest = read(&format!("{CLAUDE_ROOT}/.claude-plugin/plugin.json"));
    assert_eq!(manifest["types"], "./types/index.d.ts");
    assert!(
        workspace()
            .join(CLAUDE_ROOT)
            .join("types/index.d.ts")
            .is_file()
    );
}

#[test]
fn the_claude_code_manifest_declares_the_base_branch_and_loft_command_settings_with_their_defaults()
{
    let manifest = read(&format!("{CLAUDE_ROOT}/.claude-plugin/plugin.json"));
    let settings = &manifest["userConfig"];
    for (name, default) in [("baseBranch", "develop"), ("loftCommand", "loft")] {
        assert_eq!(settings[name]["type"], "string", "{name}");
        assert_eq!(settings[name]["default"], default, "{name}");
        assert!(settings[name]["description"].is_string(), "{name}");
    }
}

#[test]
fn the_claude_code_manifest_declares_the_yard_port_setting_with_the_page_server_s_port() {
    let manifest = read(&format!("{CLAUDE_ROOT}/.claude-plugin/plugin.json"));
    let setting = &manifest["userConfig"]["yardPort"];
    assert_eq!(setting["type"], "number");
    assert_eq!(setting["default"], 7411);
    assert!(setting["description"].is_string());
}

/// The value of `export const <name> = <value>` in Yard Client's entry module, as written.
fn mod_constant(name: &str) -> String {
    let path = format!("{CLAUDE_ROOT}/hooks/adapter/index.tsx");
    let source =
        fs::read_to_string(workspace().join(&path)).unwrap_or_else(|_| panic!("no {path}"));
    let start = format!("export const {name} = ");
    let line = source
        .lines()
        .find_map(|line| line.strip_prefix(start.as_str()))
        .unwrap_or_else(|| panic!("{path} declares no {name}"));
    line.trim_end_matches(" as const").to_owned()
}

#[test]
fn the_mod_names_the_version_of_loft_as_its_own() {
    assert_eq!(
        mod_constant("PLUGIN_VERSION"),
        format!("'{}'", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn the_newest_yard_protocol_the_mod_speaks_is_the_one_loft_speaks() {
    let scratch = Scratch::empty();
    let output = scratch.loft(&["yard", "protocol", "--format", "json"]);
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    let protocols = mod_constant("PROTOCOLS");
    let newest = protocols
        .split("newest: ")
        .nth(1)
        .and_then(|rest| rest.split(|c: char| !c.is_ascii_digit()).next())
        .unwrap_or_else(|| panic!("PROTOCOLS = {protocols} names no newest"));
    assert_eq!(newest, report["protocol"].to_string());
}

/// The command each of the Claude Code plugin's command hooks runs.
const CLAUDE_HOOK: &str = "LOFT_TELEMETRY=off LOFT_UPDATE_CHECK=off exec ${CLAUDE_PLUGIN_OPTION_LOFTCOMMAND:-loft} yard claude hook --host-pid \"$PPID\"";

#[test]
fn the_claude_code_plugins_hooks_record_holds_only_the_command_hooks_that_run_loft_yard_claude_hook()
 {
    let manifest = read(&format!("{CLAUDE_ROOT}/hooks/hooks.json"));
    let hooks = manifest["hooks"].as_object().unwrap();
    let mut names: Vec<_> = hooks.keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "PermissionRequest",
            "PostCompact",
            "PostToolUse",
            "SessionEnd",
            "SessionStart",
            "Stop",
            "SubagentStart",
            "UserPromptSubmit"
        ]
    );
    for (name, groups) in hooks {
        let groups = groups.as_array().unwrap();
        assert_eq!(groups.len(), 1, "{name}");
        let asking = name == "PermissionRequest";
        assert_eq!(
            groups[0].get("matcher"),
            asking.then(|| json!("ExitPlanMode")).as_ref(),
            "{name}"
        );
        assert_eq!(
            groups[0]["hooks"],
            json!([{"type": "command", "command": CLAUDE_HOOK, "timeout": if asking { 86_400 } else { 10 }}]),
            "{name}"
        );
    }
}

/// Runs the Claude Code plugin's hook command through the shell as Claude Code does, with the
/// `loftCommand` setting `setting` and the hook object `hook` on its standard input.
fn run_claude_hook(scratch: &Scratch, setting: &str, hook: &Value) -> std::process::Output {
    use std::io::Write as _;
    let mut command = Command::new("sh");
    command
        .args(["-c", CLAUDE_HOOK])
        .env("CLAUDE_PLUGIN_OPTION_LOFTCOMMAND", setting)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    isolate(&mut command, &scratch.home());
    let mut child = command.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(hook.to_string().as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[cfg(unix)]
#[test]
fn the_claude_hook_command_runs_a_loft_command_of_several_words_and_passes_its_claude_code_process()
{
    let scratch = Scratch::empty();
    let setting = format!("/usr/bin/env {}", env!("CARGO_BIN_EXE_loft"));
    let dir = scratch.0.canonicalize().unwrap();
    let ended =
        json!({"session_id": "s1", "cwd": dir, "hook_event_name": "SessionEnd", "reason": "clear"});
    let ran = run_claude_hook(&scratch, &setting, &ended);
    assert!(ran.status.success(), "{}", stderr(&ran));
    let started = json!({"session_id": "s2", "cwd": dir, "hook_event_name": "SessionStart", "source": "clear"});
    let ran = run_claude_hook(&scratch, &setting, &started);
    assert!(ran.status.success(), "{}", stderr(&ran));
    let continued = fs::read_to_string(scratch.home().join(".yard/continuations.json")).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&continued).unwrap(),
        json!({"s2": "s1"}),
        "both hooks ran under one parent, the process the shell names in $PPID"
    );
}

/// The events whose hooks Claude Code's built-in guard passes over for a user's mod; `classic.`
/// names every settings hook event.
const GUARDED: [&str; 9] = [
    "classic.",
    "prompt.compose",
    "prompt.section",
    "prompt.context",
    "prompt.attachment",
    "skill.prompt",
    "attribution.text",
    "session.compact",
    "settings.read",
];

/// The `session.append` rows the guard keeps from a user's mod, by the `door` a hook filters to.
const GUARDED_DOORS: [&str; 3] = ["attachment", "hook-context", "compaction"];

/// Each hook `source` registers on an event the guard passes over, as `on('<event>'` names it.
fn guarded_hooks(source: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (at, _) in source.match_indices("on(") {
        let rest = &source[at + 3..];
        let Some(quote) = rest
            .chars()
            .next()
            .filter(|c| matches!(c, '\'' | '"' | '`'))
        else {
            continue;
        };
        let Some(name) = rest[1..].split(quote).next() else {
            continue;
        };
        let filter = rest[1 + name.len()..].split(')').next().unwrap_or_default();
        let door = GUARDED_DOORS
            .iter()
            .any(|door| filter.contains(&format!("door: '{door}'")));
        if GUARDED.iter().any(|guarded| name.starts_with(guarded))
            || (name == "session.append" && door)
        {
            found.push(name.to_owned());
        }
    }
    found
}

#[test]
fn a_hook_on_an_event_the_guard_passes_over_is_found() {
    let source = "on('turn.start', f)\non('classic.Stop', f)\non(\"session.compact\", f)\non('session.append', { door: 'hook-context' }, f)\non('session.append', f)";
    assert_eq!(
        guarded_hooks(source),
        ["classic.Stop", "session.compact", "session.append"]
    );
}

#[test]
fn the_mod_registers_no_hook_on_an_event_claude_code_s_guard_passes_over() {
    let mut sources = vec![workspace().join(CLAUDE_ROOT).join("hooks")];
    let mut found = Vec::new();
    while let Some(path) = sources.pop() {
        if path.is_dir() {
            sources.extend(
                fs::read_dir(&path)
                    .unwrap()
                    .map(|entry| entry.unwrap().path()),
            );
        } else if path
            .extension()
            .is_some_and(|extension| extension == "ts" || extension == "tsx")
        {
            let source = fs::read_to_string(&path).unwrap();
            found.extend(
                guarded_hooks(&source)
                    .into_iter()
                    .map(|name| format!("{}: {name}", path.display())),
            );
        }
    }
    assert_eq!(found, Vec::<String>::new());
}

/// The oldest Claude Code the mod runs under, as the Loft mod's Component states it.
const OLDEST_CLAUDE_CODE: [u32; 3] = [2, 1, 287];

#[test]
fn ci_tests_the_mod_under_a_pinned_claude_code_and_the_page_in_a_dom_emulator() {
    let ci = fs::read_to_string(workspace().join(".github/workflows/ci.yml")).unwrap();
    let plugin = job(&ci, "plugin").runs();
    let install = plugin
        .iter()
        .find_map(|run| run.strip_prefix("npm install --global @anthropic-ai/claude-code@"))
        .expect("the plugin job installs Claude Code");
    let pinned: Vec<u32> = install
        .split('.')
        .map(|part| part.parse().expect("an exact version"))
        .collect();
    assert!(
        pinned.as_slice() >= OLDEST_CLAUDE_CODE.as_slice(),
        "{install}"
    );
    let script = plugin.join("\n");
    for command in [
        "claude plugin validate plugins/claude/loft",
        "claude plugin test plugins/claude/loft",
    ] {
        assert!(script.lines().any(|line| line == command), "{command}");
    }
    assert_eq!(
        job(&ci, "page").runs(),
        ["npm ci", "npm test", "npm run types"]
    );
}

/// The subcommands `loft --help` lists, in its order, less `help`.
fn loft_subcommands() -> Vec<String> {
    let output = Scratch::empty().loft(&["--help"]);
    let help = String::from_utf8(output.stdout).unwrap();
    help.lines()
        .skip_while(|line| *line != "Commands:")
        .skip(1)
        .take_while(|line| line.starts_with("  "))
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| *name != "help")
        .map(ToOwned::to_owned)
        .collect()
}

#[test]
fn the_mod_takes_a_subcommand_s_name_only_from_loft_s_own_list_of_subcommands() {
    let path = format!("{CLAUDE_ROOT}/hooks/adapter/shell.ts");
    let source =
        fs::read_to_string(workspace().join(&path)).unwrap_or_else(|_| panic!("no {path}"));
    let listed: Vec<String> = source
        .split("export const LOFT_SUBCOMMANDS: readonly string[] = [")
        .nth(1)
        .and_then(|rest| rest.split(']').next())
        .unwrap_or_else(|| panic!("{path} declares no LOFT_SUBCOMMANDS"))
        .split(',')
        .map(|name| name.trim().trim_matches('\'').to_owned())
        .filter(|name| !name.is_empty())
        .collect();
    assert_eq!(listed, loft_subcommands());
}
