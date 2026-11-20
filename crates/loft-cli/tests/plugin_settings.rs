use std::fs;

use serde_json::Value;

use crate::common::workspace;

#[test]
fn the_claude_code_manifest_declares_the_yard_listen_setting_empty_by_default() {
    let path = workspace().join("plugins/claude/loft/.claude-plugin/plugin.json");
    let manifest: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    let setting = &manifest["userConfig"]["yardListen"];
    assert_eq!(setting["type"], "string");
    assert_eq!(setting["default"], "");
    assert!(setting["description"].is_string());
}
