//! The `loft-spec` skill's source holds to its own rules: it names no host outside `agents/`,
//! each reference opens with its use and lists its sections, the router routes every reference,
//! and every YAML example runs, a tree passing `loft check` and a wrong example raising its rule.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::common::{Scratch, skill_source, stdout};
use crate::markdown::{rows_by_span, table, tables};
use loftspec::lint::RuleId;
use serde_json::Value;

fn text(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|_| panic!("no {}", path.display()))
}

/// The references, by file name, with their text.
fn references() -> BTreeMap<String, String> {
    let found: BTreeMap<String, String> = fs::read_dir(skill_source().join("references"))
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
pub fn split(skill_md: &str) -> (Vec<&str>, Vec<&str>) {
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

/// The JSON document of `loft reconcile` over a tree `loft init` created, `files` written into
/// it and `loft fmt` run: an example trace file names keys of its own, so only its form is
/// checked.
fn reconciled(files: &[(String, String)]) -> Value {
    let scratch = Scratch::empty();
    assert_eq!(scratch.loft(&["init", "--no-skill"]).status.code(), Some(0));
    for (path, text) in files {
        let at = scratch.0.join(path);
        fs::create_dir_all(at.parent().unwrap()).unwrap();
        fs::write(at, text).unwrap();
    }
    assert_eq!(scratch.loft(&["fmt"]).status.code(), Some(0), "{files:?}");
    serde_json::from_str(&stdout(&scratch.loft(&["reconcile", "--format", "json"]))).unwrap()
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
    let mut neutral = vec![(
        "SKILL.md".to_owned(),
        text(&skill_source().join("SKILL.md")),
    )];
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
            skill_source().join("agents").join(adaptation).is_file(),
            "{adaptation}"
        );
    }
    assert!(!skill_source().join("references/claude-host.md").exists());
}

#[test]
fn the_claude_code_fragment_pre_approves_only_the_commands_that_write_nothing() {
    let fragment = text(&skill_source().join("agents/claude/frontmatter.yaml"));
    let tools = fragment
        .lines()
        .find_map(|line| line.strip_prefix("allowed-tools: "))
        .unwrap();
    assert_eq!(
        tools.split(", ").collect::<Vec<_>>(),
        [
            "Bash(loft --version)",
            "Bash(loft check:*)",
            "Bash(loft fmt --check:*)",
            "Bash(loft verify:*)",
            "Bash(loft find:*)",
            "Bash(loft terms:*)",
            "Bash(loft status:*)",
            "Bash(loft review:*)",
        ]
    );
}

#[test]
fn the_frontmatter_holds_the_name_and_the_description_alone() {
    let skill_md = text(&skill_source().join("SKILL.md"));
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
fn the_description_names_writing_changing_and_reviewing_a_specification_with_loft() {
    let skill_md = text(&skill_source().join("SKILL.md"));
    let (frontmatter, _) = split(&skill_md);
    let description = frontmatter[1].strip_prefix("description: ").unwrap();
    let (first, _) = description.split_once(". ").unwrap();
    let words: BTreeSet<String> = first
        .split(|c: char| !c.is_alphanumeric())
        .map(str::to_lowercase)
        .collect();
    for word in ["write", "change", "review", "specification"] {
        assert!(words.contains(word), "{word}: {first}");
    }
    assert!(first.contains("`loft`"), "{first}");
}

#[test]
fn the_router_routes_each_reference_once_and_the_loop_first() {
    let skill_md = text(&skill_source().join("SKILL.md"));
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
        // Only a column-0 "```yaml" fence is read as an example, so every fence takes that form.
        for line in markdown.lines() {
            let fence = line.trim_start();
            if fence.starts_with("```") || fence.starts_with("~~~") {
                let read = line == "```" || line.starts_with("```yaml");
                assert!(read, "{name}: a fence the examples cannot read: `{line}`");
            }
        }
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
        if examples
            .tree
            .iter()
            .any(|(path, _)| path == "loft.trace.yaml")
        {
            let reconciled = reconciled(&examples.tree);
            assert_eq!(reconciled["errors"], Value::Array(Vec::new()), "{name}");
        }
    }
    assert!(trees >= 2, "the scan found {trees} example trees");
}

#[test]
fn each_wrong_example_raises_its_rule_and_no_other_warning() {
    for (name, markdown) in references() {
        let examples = examples(&markdown);
        // Each example runs `loft` three times in its own scratch directory, so they run at once.
        std::thread::scope(|scope| {
            for (rule, files) in &examples.wrong {
                let (name, tree) = (&name, &examples.tree);
                scope.spawn(move || raises_only(name, rule, tree, files));
            }
        });
    }
}

/// Asserts that `files`, written over `tree`, raise `rule` and no other warning.
fn raises_only(name: &str, rule: &str, tree: &[(String, String)], files: &[(String, String)]) {
    let mut tree = tree.to_vec();
    tree.extend(files.iter().cloned());
    let document = checked(&tree);
    assert!(rules(&document, None).contains(rule), "{name}: {rule}");
    let mut warnings = rules(&document, Some("warning"));
    warnings.remove(rule);
    assert_eq!(warnings, BTreeSet::new(), "{name}: {rule}");
}

/// The rules an unpublished tree cannot raise under `loft check`: a Quality Attribute always
/// holds a target, the label rules need labels, the rules of decision history need what a
/// publish records, and only `loft terms` runs the term report's.
const UNRAISABLE: [&str; 8] = [
    "LQ-006", "V2-003", "V2-004", "V2-010", "V2-011", "TD-001", "TD-002", "TD-003",
];

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
            matches!(prefix, "SH" | "LQ" | "V2" | "TD") && number.len() == 3
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

/// Whether a cell says something: a dash or an empty cell does not.
fn says(cell: &str) -> bool {
    cell.chars().any(char::is_alphanumeric)
}

#[test]
fn each_rule_is_given_its_meaning_and_its_usual_fix() {
    let guide = &references()["lint-findings.md"];
    let tables: Vec<_> = tables(guide)
        .into_iter()
        .filter(|table| table.header == ["Rule", "Default", "Reports", "Usual fix"])
        .collect();
    let rows: BTreeMap<&str, &[String]> = tables.iter().flat_map(rows_by_span).collect();
    for rule in RuleId::ALL {
        let row = rows
            .get(rule.as_str())
            .unwrap_or_else(|| panic!("no row for {}", rule.as_str()));
        for cell in &row[2..] {
            assert!(says(cell), "{}: {cell:?}", rule.as_str());
        }
    }
}

#[test]
fn each_error_kind_of_the_check_schema_is_described_with_its_usual_fix() {
    let schema: Value = serde_json::from_str(include_str!("../../../schemas/check.json")).unwrap();
    let kinds: BTreeSet<&str> = schema["$defs"]["error"]["properties"]["kind"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|kind| kind.as_str().unwrap())
        .collect();
    let guide = &references()["lint-findings.md"];
    let errors = table(guide, "`kind`");
    assert_eq!(errors.header[1..], ["Means", "Usual fix"]);
    let rows = rows_by_span(&errors);
    assert_eq!(rows.keys().copied().collect::<BTreeSet<_>>(), kinds);
    for (kind, row) in rows {
        assert!(row[1..].iter().all(|cell| says(cell)), "{kind}: {row:?}");
    }
}

#[test]
fn a_fragment_block_is_shown_and_not_run() {
    // A fragment that would neither read nor check as a file of the tree.
    let markdown = "Text.\n\n```yaml fragment\nmentions:\n  - name: \"Nothing\"\n```\n";
    let examples = examples(markdown);
    assert_eq!(examples.tree, Vec::new());
    assert_eq!(examples.wrong, BTreeMap::new());
    let shown = references()
        .values()
        .flat_map(|markdown| yaml_blocks(markdown))
        .filter(|block| block.info == "fragment")
        .count();
    assert!(shown >= 1, "no reference shows a fragment");
}
