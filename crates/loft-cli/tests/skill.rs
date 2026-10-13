//! The `loft-spec` skill's source holds to its own rules: it names no host outside `agents/`,
//! each reference opens with its use and lists its sections, the router routes every reference,
//! and every YAML example runs, a tree passing `loft check` and a wrong example raising its rule.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::common::{Scratch, stdout};
use loftspec::lint::RuleId;
use serde_json::Value;

fn skill() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("skill/loft-spec")
}

fn text(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|_| panic!("no {}", path.display()))
}

/// The references, by file name, with their text.
fn references() -> BTreeMap<String, String> {
    let found: BTreeMap<String, String> = fs::read_dir(skill().join("references"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .map(|path| {
            (
                path.file_name().unwrap().to_string_lossy().into_owned(),
                text(&path),
            )
        })
        .collect();
    assert!(
        found.len() >= 8,
        "the scan found {} references",
        found.len()
    );
    found
}

/// The frontmatter's lines and the body's, fences excluded.
fn split(skill_md: &str) -> (Vec<&str>, Vec<&str>) {
    let lines: Vec<&str> = skill_md.lines().collect();
    assert_eq!(lines[0], "---", "SKILL.md opens with a frontmatter");
    let end = lines[1..].iter().position(|line| *line == "---").unwrap() + 1;
    (lines[1..end].to_vec(), lines[end + 1..].to_vec())
}

/// A fenced YAML block: its info string after `yaml`, and its text.
struct Block {
    info: String,
    text: String,
}

fn yaml_blocks(markdown: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut lines = markdown.lines();
    while let Some(line) = lines.next() {
        let Some(info) = line.strip_prefix("```yaml") else {
            continue;
        };
        let body: Vec<&str> = lines.by_ref().take_while(|line| *line != "```").collect();
        blocks.push(Block {
            info: info.trim().to_owned(),
            text: body.join("\n") + "\n",
        });
    }
    blocks
}

/// The value of `name=` in an info string.
fn attribute<'a>(info: &'a str, name: &str) -> Option<&'a str> {
    info.split_whitespace()
        .find_map(|part| part.strip_prefix(name)?.strip_prefix('='))
}

/// A reference's examples: its tree, and each wrong example's files by rule.
struct Examples {
    tree: Vec<(String, String)>,
    wrong: BTreeMap<String, Vec<(String, String)>>,
}

fn examples(markdown: &str) -> Examples {
    let mut examples = Examples {
        tree: Vec::new(),
        wrong: BTreeMap::new(),
    };
    for block in yaml_blocks(markdown) {
        if block.info == "fragment" {
            continue;
        }
        let path = attribute(&block.info, "path").expect("a marked block names its path");
        let file = (path.to_owned(), block.text);
        match attribute(&block.info, "rule") {
            Some(rule) => examples
                .wrong
                .entry(rule.to_owned())
                .or_default()
                .push(file),
            None => examples.tree.push(file),
        }
    }
    examples
}

/// The JSON document of `loft check` over a tree `loft init` created, `files` written into it
/// and `loft fmt` run.
fn checked(files: &[(String, String)]) -> Value {
    let scratch = Scratch::empty();
    assert_eq!(scratch.loft(&["init", "--no-skill"]).status.code(), Some(0));
    for (path, text) in files {
        let at = scratch.0.join(path);
        fs::create_dir_all(at.parent().unwrap()).unwrap();
        fs::write(at, text).unwrap();
    }
    assert_eq!(scratch.loft(&["fmt"]).status.code(), Some(0), "{files:?}");
    serde_json::from_str(&stdout(&scratch.loft(&["check", "--format", "json"]))).unwrap()
}

/// The rules of a check document's findings at `severity`, or at any severity.
fn rules(document: &Value, severity: Option<&str>) -> BTreeSet<String> {
    document["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|finding| severity.is_none_or(|severity| finding["severity"] == severity))
        .map(|finding| finding["rule"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn the_skill_s_neutral_files_name_no_host() {
    let mut neutral = vec![("SKILL.md".to_owned(), text(&skill().join("SKILL.md")))];
    neutral.extend(references());
    for (name, text) in &neutral {
        for host in [
            "Claude",
            "claude",
            "Codex",
            "codex",
            "openai",
            "OpenAI",
            "ChatGPT",
            "$loft-spec",
        ] {
            assert!(!text.contains(host), "{name} names the host {host}");
        }
    }
}

#[test]
fn each_host_s_adaptation_lies_under_agents() {
    for adaptation in [
        "openai.yaml",
        "claude/frontmatter.yaml",
        "claude/route.md",
        "claude/claude-host.md",
    ] {
        assert!(
            skill().join("agents").join(adaptation).is_file(),
            "{adaptation}"
        );
    }
    assert!(!skill().join("references/claude-host.md").exists());
}

#[test]
fn the_frontmatter_holds_the_name_and_the_description_alone() {
    let skill_md = text(&skill().join("SKILL.md"));
    let (frontmatter, _) = split(&skill_md);
    let fields: Vec<&str> = frontmatter
        .iter()
        .map(|line| line.split_once(':').unwrap().0)
        .collect();
    assert_eq!(fields, ["name", "description"]);
    assert_eq!(frontmatter[0], "name: loft-spec");
    let description = frontmatter[1].strip_prefix("description: ").unwrap();
    assert!(description.chars().count() <= 1024, "{}", description.len());
    assert!(skill_md.lines().count() <= 500);
}

#[test]
fn the_router_routes_each_reference_once_and_the_loop_first() {
    let skill_md = text(&skill().join("SKILL.md"));
    let (_, body) = split(&skill_md);
    let bullets: Vec<&str> = body
        .iter()
        .filter(|line| line.starts_with("- Read") || line.starts_with("- Also read"))
        .copied()
        .collect();
    let routed: Vec<String> = bullets
        .iter()
        .map(|bullet| {
            let start = bullet
                .find("references/")
                .expect("a route names its reference");
            let rest = &bullet[start + "references/".len()..];
            rest[..rest.find('`').unwrap()].to_owned()
        })
        .collect();
    assert_eq!(routed[0], "the-loop.md");
    let unique: BTreeSet<&String> = routed.iter().collect();
    assert_eq!(unique.len(), routed.len(), "{routed:?}");
    let files: BTreeSet<String> = references().into_keys().collect();
    assert_eq!(unique.into_iter().cloned().collect::<BTreeSet<_>>(), files);
}

#[test]
fn each_reference_opens_with_its_use_and_lists_its_sections_in_order() {
    for (name, markdown) in references() {
        let paragraph = markdown
            .lines()
            .find(|line| !line.starts_with('#') && !line.is_empty())
            .unwrap();
        assert!(paragraph.starts_with("Use this workflow for"), "{name}");
        let (_, after) = markdown.split_once("## Contents\n").expect("## Contents");
        let listed: Vec<&str> = after
            .lines()
            .skip_while(|line| line.is_empty())
            .take_while(|line| line.starts_with("- "))
            .map(|line| &line[2..])
            .collect();
        let sections: Vec<&str> = after
            .lines()
            .filter_map(|line| line.strip_prefix("## "))
            .collect();
        assert_eq!(listed, sections, "{name}");
    }
}

#[test]
fn every_yaml_block_is_marked_with_its_role() {
    for (name, markdown) in references() {
        for block in yaml_blocks(&markdown) {
            let marked = block.info == "fragment" || attribute(&block.info, "path").is_some();
            assert!(marked, "{name}: a YAML block marked `{}`", block.info);
        }
    }
}

#[test]
fn each_reference_s_example_tree_passes_check() {
    let mut trees = 0;
    for (name, markdown) in references() {
        let examples = examples(&markdown);
        if examples.tree.is_empty() {
            continue;
        }
        trees += 1;
        let document = checked(&examples.tree);
        assert_eq!(document["errors"], Value::Array(Vec::new()), "{name}");
        assert_eq!(rules(&document, Some("warning")), BTreeSet::new(), "{name}");
    }
    assert!(trees >= 2, "the scan found {trees} example trees");
}

#[test]
fn each_wrong_example_raises_its_rule_and_no_other_warning() {
    for (name, markdown) in references() {
        let examples = examples(&markdown);
        for (rule, files) in &examples.wrong {
            let mut tree = examples.tree.clone();
            tree.extend(files.iter().cloned());
            let document = checked(&tree);
            assert!(rules(&document, None).contains(rule), "{name}: {rule}");
            let mut warnings = rules(&document, Some("warning"));
            warnings.remove(rule);
            assert_eq!(warnings, BTreeSet::new(), "{name}: {rule}");
        }
    }
}

/// The rules an unpublished tree cannot raise: a Quality Attribute always holds a target, and
/// the label rules need labels.
const UNRAISABLE: [&str; 3] = ["LQ-006", "V2-003", "V2-004"];

#[test]
fn the_lint_guide_describes_each_rule_and_shows_each_rule_an_unpublished_tree_can_raise() {
    let guide = &references()["lint-findings.md"];
    let defined: BTreeSet<String> = RuleId::ALL
        .iter()
        .map(|rule| rule.as_str().to_owned())
        .collect();
    let cited: BTreeSet<String> = guide
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
        .filter(|word| {
            let (prefix, number) = word.split_once('-').unwrap_or_default();
            matches!(prefix, "SH" | "LQ" | "V2") && number.len() == 3
        })
        .map(str::to_owned)
        .collect();
    assert_eq!(cited, defined);
    let shown: BTreeSet<String> = examples(guide).wrong.into_keys().collect();
    let raisable: BTreeSet<String> = defined
        .into_iter()
        .filter(|rule| !UNRAISABLE.contains(&rule.as_str()))
        .collect();
    assert_eq!(shown, raisable);
}
