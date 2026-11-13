//! The repository is written in American English: each Product's Constraint says so, and this
//! guard holds every text file to it. Text the repository keeps as history or does not own keeps
//! its spelling: decisions published as resolved, which change only through a decision that
//! supersedes them; `testdata/`; vendored files; and the released entries of `CHANGELOG.md`.

use crate::common::{stderr, stdout, workspace};
use crate::markdown::section;
use loftspec::codec::{self, Files, SpecPath};
use loftspec::ids::ElementId;
use software_plant::real_git::real_git;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

/// British stems found anywhere in a word, each with its American spelling.
const STEMS: [(&str, &str); 11] = [
    ("acknowledgement", "acknowledgment"),
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
];

/// Stems whose `l` British English doubles where an inflection follows them: `labelled`,
/// `labelling`, `labeller`, but not `cancellation`, which American English spells so too. The
/// American form keeps one `l`.
const LL_STEMS: [&str; 6] = [
    "cancell", "labell", "modell", "signall", "travell", "levell",
];

/// The inflections after which a stem of `LL_STEMS` is British.
const LL_ENDINGS: [&str; 3] = ["ed", "ing", "er"];

/// Stems spelled `-ise` or `-yse` in British English, found where an inflection follows them:
/// `normalise`, `normalised`, `normalising`, `normalisation`, `normalisable`, but not `analysis`
/// or `emphasis`. The American form ends the stem in `z`.
const SE_STEMS: [&str; 40] = [
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
    "analys",
    "paralys",
    "catalys",
];

/// The inflections after which a stem of `SE_STEMS` is British.
const SE_ENDINGS: [&str; 4] = ["e", "ation", "ing", "able"];

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

/// Words that hold a British stem and pass whole: `analyses`, the plural of `analysis` in
/// either, and `labelledby`, which the attribute `aria-labelledby` fixes.
const NOT_BRITISH: [&str; 2] = ["analyses", "labelledby"];

/// Paths from the root, beyond those git ignores, that hold text the repository keeps as history
/// or does not own: what the agent host keeps, the fixtures, proptest's shrink records, the
/// vendored scripts, and this guard's own word lists.
const SKIPPED: [&str; 5] = [
    ".claude",
    "testdata",
    "crates/loftspec/proptest-regressions",
    "crates/loft-cli/yard-page/vendor",
    "crates/loft-cli/tests/american_english.rs",
];

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

/// The American spelling of `word`, lowercased, when it is British.
fn american(word: &str) -> Option<String> {
    let lower = word.to_lowercase();
    if NOT_BRITISH.contains(&lower.as_str()) {
        return None;
    }
    if let Some((_, us)) = WORDS.iter().find(|(gb, _)| *gb == lower) {
        return Some((*us).to_owned());
    }
    if let Some((gb, us)) = STEMS.iter().find(|(gb, _)| lower.contains(gb)) {
        return Some(lower.replacen(gb, us, 1));
    }
    if let Some(gb) = LL_STEMS
        .iter()
        .find(|gb| inflected(&lower, gb, &LL_ENDINGS))
    {
        return Some(lower.replacen(gb, &gb[..gb.len() - 1], 1));
    }
    let gb = SE_STEMS
        .iter()
        .find(|gb| inflected(&lower, gb, &SE_ENDINGS))?;
    Some(lower.replacen(gb, &format!("{}z", &gb[..gb.len() - 1]), 1))
}

/// Whether `stem` occurs in `word` with one of `endings` after it.
fn inflected(word: &str, stem: &str, endings: &[&str]) -> bool {
    word.find(stem).is_some_and(|at| {
        let rest = &word[at + stem.len()..];
        endings.iter().any(|ending| rest.starts_with(ending))
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

/// What the guard reads files against: the files of the decisions published as resolved, each
/// as a path from the root, and what it has already found each word to be.
#[derive(Default)]
struct Guard {
    published: HashSet<String>,
    seen: HashMap<String, Option<String>>,
}

impl Guard {
    /// A guard over the tree at `root`, whose decisions bound with a published history, as
    /// `V2-010` judges them, keep their words.
    fn of(root: &Path) -> Self {
        let (document, sites) = codec::read_sited(&spec_files(&root.join("spec"), "")).unwrap();
        let published = document
            .bindings
            .iter()
            .filter(|binding| binding.published_history.is_some())
            .filter_map(|binding| match binding.element {
                ElementId::Decision(id) => sites.get(id.key()),
                ElementId::Node(_) => None,
            })
            .map(|site| site.path.to_string())
            .collect();
        Self {
            published,
            ..Self::default()
        }
    }

    /// The part of `text` the guard reads, as the byte it starts at and its end: all of it, none
    /// of a decision published as resolved, and of the changelog only what is unreleased.
    fn guarded(&self, file: &str, text: &str) -> (usize, usize) {
        if self.published.contains(file) {
            return (0, 0);
        }
        if file == "CHANGELOG.md" {
            let unreleased = section(text, "Unreleased");
            let start = unreleased.as_ptr().addr() - text.as_ptr().addr();
            return (start, start + unreleased.len());
        }
        (0, text.len())
    }

    /// The American spelling of `word` when it is British, worked out once for each word.
    fn american(&mut self, word: &str) -> Option<&str> {
        if !self.seen.contains_key(word) {
            self.seen.insert(word.to_owned(), american(word));
        }
        self.seen[word].as_deref()
    }

    /// Each British form in `text` of `file`, as `file:line: found → American`.
    fn findings(&mut self, file: &str, text: &str) -> Vec<String> {
        let (start, end) = self.guarded(file, text);
        let first = text[..start].lines().count();
        let mut found = Vec::new();
        for (number, line) in text[start..end].lines().enumerate() {
            if ALLOWED.contains(&(file, line.trim())) {
                continue;
            }
            for word in words(line) {
                let us = self.american(word);
                found.extend(us.map(|us| format!("{file}:{}: {word} → {us}", first + number + 1)));
            }
        }
        found
    }
}

/// Every file of the `spec/` tree at `dir`, by its path under `spec/` after `prefix`, read as
/// they lie: unlike a store's, this read finishes no interrupted save in the working tree.
fn spec_files(dir: &Path, prefix: &str) -> Files {
    let mut files = Files::new();
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let name = format!("{prefix}{}", entry.file_name().to_string_lossy());
        if entry.file_type().unwrap().is_dir() {
            files.extend(spec_files(&entry.path(), &format!("{name}/")));
        } else if Path::new(&name).extension().is_some_and(|e| e == "yaml") {
            files.insert(SpecPath::new(&name), fs::read(entry.path()).unwrap());
        }
    }
    files
}

/// Whether `path`, from the root, lies in one of the paths `SKIPPED` names.
fn skipped(path: &str) -> bool {
    SKIPPED.iter().any(|skipped| {
        path.strip_prefix(skipped)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
    })
}

/// Every file of the repository that git keeps or would add, as a path from the root, but those
/// `SKIPPED` names: what any `.gitignore` or git's own exclude file names is left out.
fn files() -> Vec<String> {
    let listed = real_git()
        .arg("-C")
        .arg(workspace())
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .output()
        .unwrap();
    assert!(listed.status.success(), "{}", stderr(&listed));
    stdout(&listed)
        .split('\0')
        .filter(|path| !path.is_empty() && !skipped(path))
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_repository_is_written_in_american_english() {
    let root = workspace();
    let mut guard = Guard::of(&root);
    let mut read = 0;
    let mut found = Vec::new();
    for path in files() {
        // A symbolic link is read where it leads, such as each `CLAUDE.md` beside its `AGENTS.md`.
        let full = root.join(&path);
        if fs::symlink_metadata(&full).is_ok_and(|metadata| metadata.is_symlink()) {
            continue;
        }
        // A file that is not UTF-8 is a binary one: a font, an image or a compressed script. One
        // deleted but not yet staged reads as nothing.
        let Ok(text) = fs::read_to_string(&full) else {
            continue;
        };
        read += 1;
        found.extend(guard.findings(&path, &text));
    }
    assert!(read > 500, "the scan read {read} files");
    assert_eq!(found, Vec::<String>::new());
}

#[test]
fn british_forms_are_found_in_prose_and_identifiers_alike() {
    let text = "The colour is grey.\nlet wordmarkColours = normalise(x);\n// No acknowledgement.\n";
    assert_eq!(
        Guard::default().findings("a.js", text),
        [
            "a.js:1: colour → color",
            "a.js:1: grey → gray",
            "a.js:2: Colours → colors",
            "a.js:2: normalise → normalize",
            "a.js:3: acknowledgement → acknowledgment",
        ]
    );
}

#[test]
fn american_forms_and_words_like_them_pass() {
    let text = "Cancellations, two analyses, the emphasis, a specialist, an organism, a parameter.\n\
                'aria-labelledby': 'title'";
    assert_eq!(
        Guard::default().findings("a.md", text),
        Vec::<String>::new()
    );
}

#[test]
fn a_british_form_before_able_or_a_doubled_l_before_its_inflection_is_found() {
    let text = "normalisable, serialisable, analysable\nUNLABELLED, travelling, modeller";
    assert_eq!(
        Guard::default().findings("a.rs", text),
        [
            "a.rs:1: normalisable → normalizable",
            "a.rs:1: serialisable → serializable",
            "a.rs:1: analysable → analyzable",
            "a.rs:2: UNLABELLED → unlabeled",
            "a.rs:2: travelling → traveling",
            "a.rs:2: modeller → modeler",
        ]
    );
}

#[test]
fn an_unpublished_decision_holding_a_british_form_is_reported() {
    let text = "key: k1\ntitle: \"The colour of a stage\"\n";
    assert_eq!(
        Guard::default().findings("spec/decisions/d.yaml", text),
        ["spec/decisions/d.yaml:2: colour → color"]
    );
}

#[test]
fn a_decision_published_as_resolved_keeps_its_spelling() {
    let mut guard = Guard {
        published: HashSet::from(["spec/decisions/d.yaml".to_owned()]),
        ..Guard::default()
    };
    let text = "key: k1\ntitle: \"The colour of a stage\"\n";
    assert_eq!(
        guard.findings("spec/decisions/d.yaml", text),
        Vec::<String>::new()
    );
}

#[test]
fn the_repositorys_published_decisions_are_those_bound_with_a_published_history() {
    let guard = Guard::of(&workspace());
    let leads = "spec/decisions/v2-leads-the-behaviour-it-shares-with-v1.yaml";
    assert!(guard.published.contains(leads));
    assert!(
        guard.published.len() > 10,
        "{} published",
        guard.published.len()
    );
}

#[test]
fn only_the_unreleased_entries_of_the_changelog_are_read() {
    let text = "# Changelog\n\n## 2.0.0-alpha.1\n\nA colour.\n\n## Unreleased\n\n- A colour.\n\n## 1.0.0\n\nA colour.\n";
    assert_eq!(
        Guard::default().findings("CHANGELOG.md", text),
        ["CHANGELOG.md:9: colour → color"]
    );
}

#[test]
fn the_files_git_ignores_and_the_skipped_paths_are_not_read() {
    let files = files();
    assert!(files.iter().any(|path| path == "AGENTS.md"));
    let read: Vec<&String> = (files.iter())
        .filter(|path| {
            path.starts_with("target/")
                || path.starts_with(".yard/")
                || path.starts_with("testdata/")
                || path.contains("node_modules/")
                || *path == "docs/spec.md"
        })
        .collect();
    assert_eq!(read, Vec::<&String>::new());
    assert!(skipped("crates/loft-cli/tests/american_english.rs"));
    assert!(!skipped("testdata.rs"));
    assert!(!skipped(".claudex"));
}
