//! The plugins for Claude Code and Codex: each committed package equals the composition `loft`
//! writes for its host, each marketplace lists the plugin `loft` at its host's plugin root, and
//! every manifest carries the version of `loft`; the Claude Code plugin also names the hooks
//! module and the type contract of the Loft status pane, and declares its settings.

use std::fs;

use crate::common::workspace;
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
    assert_eq!(marketplace["name"], "loft");
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
    assert_eq!(marketplace["name"], "loft");
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
fn the_claude_code_plugins_hooks_json_holds_an_empty_hooks_record_for_a_host_without_modules() {
    let hooks = read(&format!("{CLAUDE_ROOT}/hooks/hooks.json"));
    assert_eq!(hooks["hooks"], json!({}));
}
