//! Selected-host installation and preservation of the repository's live skill source.

use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

use serde_json::{Value, json};

use crate::common::{Scratch, skill_source, stderr, stdout};
use crate::schema;
use crate::skill_install::{CLAUDE, CODEX};
use crate::snapshot::snapshot;

const SOURCE: &str = "crates/loft-cli/skill/loft-spec";
const LINK: &str = "../../crates/loft-cli/skill/loft-spec";

fn installed(scratch: &Scratch, args: &[&str]) -> Value {
    let mut command = vec!["skill", "install"];
    command.extend_from_slice(args);
    command.extend_from_slice(&["--format", "json"]);
    let output = scratch.loft(&command);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    schema::conforming(&output.stdout, include_str!("../../../schemas/skill.json"))
}

fn with_source() -> Scratch {
    let scratch = Scratch::empty();
    let source = scratch.0.join(SOURCE);
    fs::create_dir_all(source.join("agents")).unwrap();
    for path in ["SKILL.md", "agents/openai.yaml"] {
        fs::copy(skill_source().join(path), source.join(path)).unwrap();
    }
    fs::write(source.join("local.md"), "unreleased local content").unwrap();
    fs::create_dir_all(scratch.0.join(".agents/skills")).unwrap();
    symlink(LINK, scratch.0.join(CODEX)).unwrap();
    scratch
}

#[test]
fn selecting_codex_preserves_claude_files_and_does_not_enable_its_plugin() {
    let scratch = Scratch::empty();
    fs::create_dir_all(scratch.0.join(CLAUDE)).unwrap();
    fs::write(scratch.0.join(CLAUDE).join("mine.md"), "kept").unwrap();
    fs::write(scratch.0.join(".claude/settings.json"), "not JSON").unwrap();
    let before = snapshot(&scratch.0.join(".claude"));
    assert_eq!(
        installed(&scratch, &["--host", "codex"])["skills"],
        json!([CODEX])
    );
    assert_eq!(snapshot(&scratch.0.join(".claude")), before);
    let fresh = Scratch::empty();
    installed(&fresh, &["--host", "codex"]);
    assert!(fresh.0.join(CODEX).join("SKILL.md").is_file());
    assert!(!fresh.0.join(".claude").exists());
}

#[test]
fn selecting_claude_code_leaves_even_an_invalid_codex_destination_untouched() {
    let scratch = Scratch::empty();
    fs::create_dir_all(scratch.0.join(".agents/skills")).unwrap();
    symlink("missing", scratch.0.join(CODEX)).unwrap();
    let result = installed(&scratch, &["--host", "claude-code"]);
    assert_eq!(result["skills"], json!([CLAUDE]));
    assert!(scratch.0.join(CLAUDE).join("SKILL.md").is_file());
    assert!(scratch.0.join(".claude/settings.json").is_file());
    assert_eq!(
        fs::read_link(scratch.0.join(CODEX)).unwrap(),
        Path::new("missing")
    );
}

#[test]
fn selecting_codex_does_not_inspect_an_invalid_claude_destination() {
    let scratch = Scratch::empty();
    fs::write(scratch.0.join(".claude"), "a file").unwrap();
    installed(&scratch, &["--host", "codex"]);
    assert_eq!(
        fs::read_to_string(scratch.0.join(".claude")).unwrap(),
        "a file"
    );
}

#[test]
fn selecting_all_installs_the_same_hosts_as_omitting_the_option() {
    for args in [&[][..], &["--host", "all"][..]] {
        let scratch = Scratch::empty();
        let result = installed(&scratch, args);
        assert_eq!(result["skills"], json!([CLAUDE, CODEX]));
        for host in [CLAUDE, CODEX] {
            assert!(scratch.0.join(host).join("SKILL.md").is_file());
        }
    }
}

#[test]
fn an_unknown_skill_host_is_a_usage_error_without_writes() {
    let scratch = Scratch::empty();
    let before = snapshot(&scratch.0);
    let output = scratch.loft(&["skill", "install", "--host", "unknown"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(snapshot(&scratch.0), before);
}

#[test]
fn repeated_installs_preserve_the_repository_discovery_link_and_source_bytes() {
    let scratch = with_source();
    let before = snapshot(&scratch.0);
    for _ in 0..2 {
        assert_eq!(
            installed(&scratch, &["--host", "codex"])["skills"],
            json!([])
        );
        assert_eq!(
            fs::read_link(scratch.0.join(CODEX)).unwrap(),
            Path::new(LINK)
        );
        assert_eq!(snapshot(&scratch.0), before);
    }
    let output = scratch.loft(&["skill", "install", "--host", "codex"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(stdout(&output).contains(&format!("unchanged {CODEX}")));
    assert!(!stdout(&output).contains("commit "));
}

#[test]
fn default_install_and_init_preserve_the_link_while_installing_claude() {
    let scratch = with_source();
    assert_eq!(installed(&scratch, &[])["skills"], json!([CLAUDE]));
    assert_eq!(
        fs::read_link(scratch.0.join(CODEX)).unwrap(),
        Path::new(LINK)
    );
    let output = scratch.loft(&["init", "--no-plugin", "--format", "json"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(document["skills"], json!([CLAUDE]));
    assert_eq!(
        fs::read_link(scratch.0.join(CODEX)).unwrap(),
        Path::new(LINK)
    );
}

#[test]
fn broken_and_unexpected_links_are_refused_before_any_installation() {
    for target in ["missing", "../../foreign"] {
        let scratch = Scratch::empty();
        fs::create_dir_all(scratch.0.join(".agents/skills")).unwrap();
        fs::create_dir_all(scratch.0.join("foreign")).unwrap();
        fs::write(scratch.0.join("foreign/mine.md"), "kept").unwrap();
        symlink(target, scratch.0.join(CODEX)).unwrap();
        let output = scratch.loft(&["skill", "install"]);
        assert_eq!(output.status.code(), Some(1));
        assert!(stderr(&output).contains(CODEX), "{}", stderr(&output));
        assert_eq!(
            fs::read_link(scratch.0.join(CODEX)).unwrap(),
            Path::new(target)
        );
        assert_eq!(
            fs::read_to_string(scratch.0.join("foreign/mine.md")).unwrap(),
            "kept"
        );
        assert!(!scratch.0.join(".claude").exists());
        assert_eq!(
            fs::read_dir(scratch.0.join(".agents/skills"))
                .unwrap()
                .count(),
            1
        );
    }
}

#[test]
fn the_known_link_requires_its_source_router_and_codex_metadata() {
    for path in ["SKILL.md", "agents/openai.yaml"] {
        let scratch = with_source();
        fs::remove_file(scratch.0.join(SOURCE).join(path)).unwrap();
        let output = scratch.loft(&["skill", "install"]);
        assert_eq!(output.status.code(), Some(1));
        assert!(stderr(&output).contains(CODEX), "{}", stderr(&output));
        assert_eq!(
            fs::read_link(scratch.0.join(CODEX)).unwrap(),
            Path::new(LINK)
        );
        assert!(!scratch.0.join(".claude").exists());
    }
}
