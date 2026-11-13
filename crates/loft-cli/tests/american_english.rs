//! The repository is written in American English: each Product's Constraint says so, and this
//! guard holds every text file to it. Text the repository keeps as history or does not own keeps
//! its spelling: decisions published as resolved, which change only through a decision that
//! supersedes them; `testdata/`; vendored files; and the released entries of `CHANGELOG.md`.

use crate::common::{shown, workspace};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// British stems found anywhere in a word, each with its American spelling.
const STEMS: [(&str, &str); 16] = [
    ("colour", "color"),
    ("behaviour", "behavior"),
    ("neighbour", "neighbor"),
    ("favour", "favor"),
    ("honour", "honor"),
    ("flavour", "flavor"),
    ("judgement", "judgment"),
    ("catalogue", "catalog"),
    ("licence", "license"),
    ("artefact", "artifact"),
    ("cancell", "cancel"),
    ("labell", "label"),
    ("modell", "model"),
    ("signall", "signal"),
    ("travell", "travel"),
    ("levell", "level"),
];

/// Stems spelled `-ise` or `-yse` in British English, found where an inflection follows them:
/// `normalise`, `normalised`, `normalising`, `normalisation`, but not `analysis` or `emphasis`.
const ISE_STEMS: [&str; 37] = [
    "emphasis",
    "recognis",
    "normalis",
    "serialis",
    "initialis",
    "authoris",
    "organis",
    "summaris",
    "customis",
    "optimis",
    "prioritis",
    "minimis",
    "maximis",
    "categoris",
    "standardis",
    "finalis",
    "materialis",
    "synchronis",
    "utilis",
    "realis",
    "characteris",
    "specialis",
    "canonicalis",
    "generalis",
    "visualis",
    "memoris",
    "tokenis",
    "sanitis",
    "apologis",
    "criticis",
    "parametris",
    "randomis",
    "centralis",
    "localis",
    "personalis",
    "stabilis",
    "capitalis",
];

/// Stems spelled `-yse` in British English.
const YSE_STEMS: [&str; 3] = ["analys", "paralys", "catalys"];

/// British words matched whole, each with its American spelling.
const WORDS: [(&str, &str); 20] = [
    ("centre", "center"),
    ("centres", "centers"),
    ("centred", "centered"),
    ("grey", "gray"),
    ("greys", "grays"),
    ("greyed", "grayed"),
    ("whilst", "while"),
    ("amongst", "among"),
    ("programme", "program"),
    ("programmes", "programs"),
    ("fulfil", "fulfill"),
    ("fulfils", "fulfills"),
    ("fulfilment", "fulfillment"),
    ("learnt", "learned"),
    ("spelt", "spelled"),
    ("enrol", "enroll"),
    ("enrolment", "enrollment"),
    ("defence", "defense"),
    ("offence", "offense"),
    ("metre", "meter"),
];

/// Paths from the root that hold text the repository keeps as history, does not own, or never
/// commits.
const SKIPPED: [&str; 17] = [
    "crates/loftspec/proptest-regressions",
    ".git",
    ".yard",
    ".loft",
    ".claude",
    ".mcp.json",
    "target",
    "testdata",
    "work-plans",
    "docs/spec.md",
    "docs/spec.json",
    "plugins/.claude-plugin",
    "plugins/claude/loft/.claude-plugin/types",
    "crates/loft-cli/yard-page/vendor",
    "crates/loft-cli/yard-page-tests/test-results",
    "crates/loft-cli/yard-page-tests/playwright-report",
    "crates/loft-cli/tests/american_english.rs",
];

/// Directory names skipped wherever they lie.
const SKIPPED_NAMES: [&str; 3] = ["node_modules", "__pycache__", ".DS_Store"];

/// Text that must keep a British form, each as its file and the line's text: the alias by which
/// the Codex ledger still reads what an older `loft` wrote, and the test that holds it to that;
/// the deprecated former name of a public function of `loftspec`; and the text of the small
/// document whose canonical bytes and hashes the golden vectors under `testdata/` record.
const ALLOWED: [(&str, &str); 4] = [
    (
        "crates/loft-cli/src/yard/codex/notification_state.rs",
        "#[serde(alias = \"cancelled\")]",
    ),
    (
        "crates/loft-cli/src/yard/codex/notification_state/tests.rs",
        "let older: Progress = serde_json::from_str(\"\\\"cancelled\\\"\").unwrap();",
    ),
    (
        "crates/loftspec/src/ids.rs",
        "pub const fn labelled(prefix: LabelPrefix, key: Key) -> Self {",
    ),
    (
        "crates/loftspec/tests/common/mod.rs",
        "definition: \"Behaviour every implementation must reproduce exactly.\".to_owned(),",
    ),
];

/// Names a standard fixes in their British spelling: the attribute `aria-labelledby`.
const STANDARD: [&str; 1] = ["labelledby"];

/// The American spelling of `word`, lowercased, when it is British.
fn american(word: &str) -> Option<String> {
    let lower = word.to_lowercase();
    if STANDARD.contains(&lower.as_str()) {
        return None;
    }
    if let Some((_, us)) = WORDS.iter().find(|(gb, _)| *gb == lower) {
        return Some((*us).to_owned());
    }
    for (gb, us) in STEMS {
        if let Some(at) = lower.find(gb) {
            let rest = &lower[at + gb.len()..];
            if gb == "cancell" && rest.starts_with("ation") {
                continue;
            }
            return Some(lower.replacen(gb, us, 1));
        }
    }
    let stems = ISE_STEMS.iter().map(|gb| (gb, "iz"));
    let (gb, us_tail) = stems
        .chain(YSE_STEMS.iter().map(|gb| (gb, "yz")))
        .find(|(gb, _)| inflected(&lower, gb))?;
    let us = format!("{}{us_tail}", &gb[..gb.len() - 2]);
    Some(lower.replacen(gb, &us, 1))
}

/// Whether `stem` occurs in `word` with an inflection after it: `e`, `ation` or `ing`.
fn inflected(word: &str, stem: &str) -> bool {
    word.find(stem).is_some_and(|at| {
        let rest = &word[at + stem.len()..];
        ["e", "ation", "ing"].iter().any(|s| rest.starts_with(s))
    })
}

/// The words of `line`: runs of letters, split again where a lowercase letter meets an
/// uppercase one, so that `wordmarkColours` holds `Colours`.
fn words(line: &str) -> Vec<&str> {
    let mut found = Vec::new();
    for run in line.split(|c: char| !c.is_ascii_alphabetic()) {
        let mut start = 0;
        let bytes = run.as_bytes();
        for at in 1..bytes.len() {
            if bytes[at - 1].is_ascii_lowercase() && bytes[at].is_ascii_uppercase() {
                found.push(&run[start..at]);
                start = at;
            }
        }
        if start < run.len() {
            found.push(&run[start..]);
        }
    }
    found
}

/// The keys bound with a published history in `spec/labels.yaml`: decisions published as
/// resolved.
fn published_decisions(labels: &str) -> HashSet<String> {
    let mut keys = HashSet::new();
    let mut current = None;
    for line in labels.lines().map(str::trim) {
        if let Some(key) = line.strip_prefix("- key: ") {
            current = Some(key.to_owned());
        } else if line.starts_with("publishedHistory:") {
            keys.extend(current.clone());
        }
    }
    keys
}

/// The part of `text` the guard reads, as the byte it starts at and its end: all of it, none of a
/// decision published as resolved, and of the changelog only what is unreleased.
fn guarded(file: &str, text: &str, published: &HashSet<String>) -> (usize, usize) {
    if file.starts_with("spec/decisions/")
        && let Some(key) = text.lines().next().and_then(|l| l.strip_prefix("key: "))
        && published.contains(key.trim())
    {
        return (0, 0);
    }
    if file == "CHANGELOG.md" {
        let Some(start) = text.find("\n## Unreleased").map(|at| at + 1) else {
            return (0, 0);
        };
        let end = text[start..]
            .find("\n## ")
            .map_or(text.len(), |at| start + at + 1);
        return (start, end);
    }
    (0, text.len())
}

/// Each British form in `text` of `file`, as `file:line: found → American`.
fn findings(file: &str, text: &str, published: &HashSet<String>) -> Vec<String> {
    let (start, end) = guarded(file, text, published);
    let first = text[..start].lines().count();
    let mut found = Vec::new();
    for (number, line) in text[start..end].lines().enumerate() {
        if ALLOWED.contains(&(file, line.trim())) {
            continue;
        }
        for word in words(line) {
            if let Some(us) = american(word) {
                found.push(format!("{file}:{}: {word} → {us}", first + number + 1));
            }
        }
    }
    found
}

/// Every file beneath `dir` that the guard reads.
fn files(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let skipped = SKIPPED.contains(&shown(&path).as_str()) || SKIPPED_NAMES.contains(&&*name);
        if skipped || fs::symlink_metadata(&path).unwrap().is_symlink() {
            continue;
        }
        if path.is_dir() {
            found.extend(files(&path));
        } else {
            found.push(path);
        }
    }
    found
}

#[test]
fn the_repository_is_written_in_american_english() {
    let root = workspace();
    let labels = fs::read_to_string(root.join("spec/labels.yaml")).unwrap();
    let published = published_decisions(&labels);
    let mut read = 0;
    let mut found = Vec::new();
    for path in files(&root) {
        // A file that is not UTF-8 is a binary one: a font, an image or a compressed script.
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        read += 1;
        found.extend(findings(&shown(&path), &text, &published));
    }
    assert!(read > 500, "the scan read {read} files");
    assert_eq!(found, Vec::<String>::new());
}

#[test]
fn british_forms_are_found_in_prose_and_identifiers_alike() {
    let none = HashSet::new();
    let text = "The colour is grey.\nlet wordmarkColours = normalise(x);\n";
    assert_eq!(
        findings("a.js", text, &none),
        [
            "a.js:1: colour → color",
            "a.js:1: grey → gray",
            "a.js:2: Colours → colors",
            "a.js:2: normalise → normalize",
        ]
    );
}

#[test]
fn american_forms_and_words_like_them_pass() {
    let none = HashSet::new();
    let text = "A cancellation, an analysis, the emphasis, a specialist, an organism, a parameter.\n\
                'aria-labelledby': 'title'";
    assert_eq!(findings("a.md", text, &none), Vec::<String>::new());
}

#[test]
fn an_unpublished_decision_holding_a_british_form_is_reported() {
    let text = "key: k1\ntitle: \"The colour of a stage\"\n";
    assert_eq!(
        findings("spec/decisions/d.yaml", text, &HashSet::new()),
        ["spec/decisions/d.yaml:2: colour → color"]
    );
}

#[test]
fn a_decision_published_as_resolved_keeps_its_spelling() {
    let labels = "bindings:\n  - key: k1\n    label: ADR-1\n    publishedState: accepted\n    publishedHistory: 00\n  - key: k2\n    label: RQ-2\n";
    let published = published_decisions(labels);
    assert_eq!(published, HashSet::from(["k1".to_owned()]));
    let text = "key: k1\ntitle: \"The colour of a stage\"\n";
    assert_eq!(
        findings("spec/decisions/d.yaml", text, &published),
        Vec::<String>::new()
    );
}

#[test]
fn only_the_unreleased_entries_of_the_changelog_are_read() {
    let text = "# Changelog\n\n## 2.0.0-alpha.1\n\nA colour.\n\n## Unreleased\n\n- A colour.\n\n## 1.0.0\n\nA colour.\n";
    assert_eq!(
        findings("CHANGELOG.md", text, &HashSet::new()),
        ["CHANGELOG.md:9: colour → color"]
    );
}
