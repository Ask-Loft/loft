//! The plugins for Claude Code and Codex: each committed package equals the composition `loft`
//! writes for its host, each marketplace, `ask-loft`, lists the plugin `loft` at its host's plugin root, and
//! every manifest carries the version of `loft`; the Claude Code plugin also names the hooks
//! module and the type contract of the Loft status pane, and declares its settings, and its mod
//! names the version of `loft` and the yard protocol `loft` speaks.

use std::fs;

use crate::common::{Scratch, workspace};
use crate::skill_install::{CLAUDE, CODEX, installed};
use crate::snapshot::snapshot;
use serde_json::{Value, json};

fn read(path: &str) -> Value {
    let text = fs::read_to_string(workspace().join(path)).unwrap_or_else(|_| panic!("no {path}"));
    serde_json::from_str(&text).unwrap()
}

const CLAUDE_ROOT: &str = "plugins/claude/loft";
const CODEX_ROOT: &str = "plugins/codex/loft";

#[test]
fn each_committed_package_equals_the_composition_for_its_host() {
    let composed = installed();
    for (root, package) in [(CLAUDE_ROOT, CLAUDE), (CODEX_ROOT, CODEX)] {
        assert_eq!(
            snapshot(&workspace().join(root).join("skills/loft-spec")),
            snapshot(&composed.0.join(package)),
            "{root}: run scripts/package-skill.sh"
        );
    }
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
fn the_claude_code_plugin_holds_the_loft_spec_and_yard_skills_and_the_codex_plugin_loft_spec_alone()
{
    assert_eq!(skills(CLAUDE_ROOT), ["loft-spec", "yard"]);
    assert_eq!(skills(CODEX_ROOT), ["loft-spec"]);
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
    assert_eq!(
        manifest["$schema"],
        "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json"
    );
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
fn the_claude_code_manifest_names_the_panes_type_contract_which_lies_in_the_plugin() {
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
    let path = format!("{CLAUDE_ROOT}/hooks/yard/index.tsx");
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

#[test]
fn the_claude_code_plugins_hooks_json_holds_an_empty_hooks_record_for_a_host_without_modules() {
    let hooks = read(&format!("{CLAUDE_ROOT}/hooks/hooks.json"));
    assert_eq!(hooks["hooks"], json!({}));
}
