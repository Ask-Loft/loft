//! `loft check` (F-38): the reader, then the validator, then the linter, with an exit code a CI
//! job can gate on. In text, each error and finding is one line, errors before findings (AC-522,
//! AC-595); with `--format json`, one document on standard output (RQ-155). With `--baseline
//! <revision>`, the findings are marked against the tree where HEAD left the revision: the text
//! prints the introduced ones and a count, and `--fail-on` gates on the introduced ones alone.

use std::path::Path;
use std::process::ExitCode;

use loftspec::check::{Change, Checked, Split, check, check_against};
use loftspec::codec::{Files, LINT, SpecPath};
use loftspec::lint::Severity;
use loftspec_store::repository::GitCommand;
use serde_json::{Map, Value, json};
use sha2::{Digest as _, Sha256};

use crate::finish::Finished;
use crate::json;
use crate::out::outln;
use crate::stop::{self, Stop};
use crate::{base, version};

/// The schema `--format json` writes to, by name and version (RQ-156).
const SCHEMA: &str = "loft-check/2";

/// What `loft check` was asked for.
pub struct Options<'a> {
    pub fail_on: Option<Severity>,
    pub baseline: Option<&'a str>,
}

/// The tree's findings marked against a base, and where the base came from.
struct Compared {
    revision: String,
    commit: String,
    split: Option<Split>,
    /// The digests of the tree's and the base's `spec/lint.yaml`, and whether they agree.
    lint_config: (Option<String>, Option<String>, bool),
}

impl Compared {
    /// Whether the finding at `index` of the tree's findings was introduced; every finding is,
    /// where no split was made.
    fn introduced(&self, index: usize) -> bool {
        self.split
            .as_ref()
            .is_none_or(|split| split.changes[index] == Change::Introduced)
    }
}

/// Whether the finding at `index` counts: every finding without a base, the introduced ones
/// against one.
fn counts(compared: Option<&Compared>, index: usize) -> bool {
    compared.is_none_or(|compared| compared.introduced(index))
}

pub fn run(root: &Path, options: &Options, as_json: bool) -> Finished {
    let (checked, compared) = match attempt(root, options.baseline) {
        Ok(result) => result,
        Err(stop) => return stop.exit(SCHEMA, as_json),
    };
    let gated = checked
        .findings()
        .iter()
        .enumerate()
        .filter(|(index, _)| counts(compared.as_ref(), *index))
        .map(|(_, finding)| finding.severity);
    let failed = checked.has_errors() || json::fails_on(options.fail_on, gated);
    let outcome = if failed { "failed" } else { "passed" };
    let document = document(&checked, compared.as_ref(), outcome);
    if as_json {
        json::print(&document);
    } else {
        print(&checked, compared.as_ref());
    }
    let code = if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    };
    Finished::with(code, outcome, json::errors_of(&document))
}

/// Checks the tree under `root`, against the tree where HEAD left `baseline` where one is given.
fn attempt(root: &Path, baseline: Option<&str>) -> Result<(Checked, Option<Compared>), Stop> {
    let files = stop::read_files(root)?;
    let Some(revision) = baseline else {
        return Ok((check(&files), None));
    };
    let base = base::read(&GitCommand::default(), root, revision)?;
    let baselined =
        check_against(&files, &base.files).map_err(|errors| base::unread(&base.commit, &errors))?;
    let compared = Compared {
        revision: revision.to_owned(),
        lint_config: (digest(&files), digest(&base.files), baselined.same_config),
        commit: base.commit,
        split: baselined.split,
    };
    Ok((baselined.checked, Some(compared)))
}

/// The SHA-256 digest of a tree's `spec/lint.yaml`, where it has one.
fn digest(files: &Files) -> Option<String> {
    files
        .get(&SpecPath::new(LINT))
        .map(|bytes| format!("sha256:{:x}", Sha256::digest(bytes)))
}

fn print(checked: &Checked, compared: Option<&Compared>) {
    match checked {
        Checked::Unread(errors) => {
            for error in errors {
                outln!("{error}");
            }
        }
        Checked::Read {
            invalid,
            config,
            findings,
        } => {
            for error in invalid {
                outln!("{error}");
            }
            for error in config {
                outln!("{error}");
            }
            for (index, finding) in findings.iter().enumerate() {
                if counts(compared, index) {
                    outln!("{finding}");
                }
            }
        }
    }
    if let Some(compared) = compared {
        outln!("{}", summary(checked, compared));
    }
}

/// The line that ends a check against a base.
fn summary(checked: &Checked, compared: &Compared) -> String {
    let (introduced, preexisting, resolved) = compared.split.as_ref().map_or_else(
        || (checked.findings().len(), 0, 0),
        |split| {
            let introduced = split
                .changes
                .iter()
                .filter(|change| **change == Change::Introduced)
                .count();
            (
                introduced,
                split.changes.len() - introduced,
                split.resolved.len(),
            )
        },
    );
    format!(
        "baseline: {} at {}: {introduced} introduced, {preexisting} preexisting, {resolved} \
         resolved",
        compared.revision, compared.commit
    )
}

fn document(checked: &Checked, compared: Option<&Compared>, outcome: &str) -> Value {
    let (errors, mut findings): (Vec<Value>, Vec<Value>) = match checked {
        Checked::Unread(errors) => (json::read_errors(errors), Vec::new()),
        Checked::Read {
            invalid,
            config,
            findings,
        } => (
            json::validation_errors(invalid)
                .into_iter()
                .chain(config.iter().map(|e| json::read_error("lint-config", e)))
                .collect(),
            findings.iter().map(json::finding).collect(),
        ),
    };
    let mut fields = Map::new();
    if let Some(compared) = compared {
        for (index, finding) in findings.iter_mut().enumerate() {
            let change = if compared.introduced(index) {
                "introduced"
            } else {
                "preexisting"
            };
            finding["change"] = json!(change);
        }
        let resolved = compared.split.as_ref().map_or_else(Vec::new, |split| {
            split.resolved.iter().map(json::finding).collect()
        });
        fields.insert("resolved".to_owned(), Value::Array(resolved));
        fields.insert("baseline".to_owned(), baseline(compared));
    }
    fields.insert("findings".to_owned(), Value::Array(findings));
    json::result(SCHEMA, outcome, Value::Object(fields), errors)
}

fn baseline(compared: &Compared) -> Value {
    let (tree, base, same) = &compared.lint_config;
    json!({
        "revision": compared.revision,
        "commit": compared.commit,
        "loft": version::VERSION,
        "lintConfig": { "tree": tree, "base": base, "same": same },
    })
}
