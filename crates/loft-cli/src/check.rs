//! `loft check` (F-38): the reader, then the validator, then the linter, with an exit code a CI
//! job can gate on. In text, each error and finding is one line, errors before findings (AC-522,
//! AC-595); with `--format json`, one document on standard output (RQ-155). With `--baseline
//! <revision>`, the findings are marked against the tree where HEAD left the revision: the text
//! prints the introduced ones and a count, and `--fail-on` gates on the introduced ones alone.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::ExitCode;

use loftspec::check::{Change, Checked, LintConfigs, Split, check, check_against};
use loftspec::codec::Files;
use loftspec::identity::Key;
use loftspec::lint::{Finding, Severity};
use loftspec_store::repository::GitCommand;
use serde_json::{Map, Value, json};

use crate::finish::Finished;
use crate::json;
use crate::out::outln;
use crate::stop::{self, Stop};
use crate::{base, version};

/// The schema `--format json` writes to, by name and version (RQ-156).
const SCHEMA: &str = "loft-check/2";

const FAILED: &str = "failed";
const PASSED: &str = "passed";

/// What `loft check` was asked for.
pub struct Options<'a> {
    pub fail_on: Option<Severity>,
    pub baseline: Option<&'a str>,
}

/// The base a check compares with: the revision named, the commit where HEAD left it, and the
/// files of `spec/` there.
pub struct Baseline<'a> {
    pub revision: &'a str,
    pub commit: &'a str,
    pub files: &'a Files,
}

/// The tree's findings split against a base, and where the base came from.
struct Compared {
    revision: String,
    commit: String,
    split: Split,
    lint_configs: LintConfigs,
}

/// A tree checked, against a base where one is given: what `loft check` reports.
pub struct Report {
    checked: Checked,
    compared: Option<Compared>,
}

pub fn run(root: &Path, options: &Options, as_json: bool) -> Finished {
    let report = match attempt(root, options.baseline) {
        Ok(report) => report,
        Err(stop) => return stop.exit(SCHEMA, as_json),
    };
    let outcome = report.outcome(options.fail_on);
    let document = report.document(outcome);
    if as_json {
        json::print(&document);
    } else {
        report.print();
    }
    let code = if outcome == FAILED {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    };
    Finished::with(code, outcome, json::errors_of(&document))
}

/// Checks the tree under `root`, against the tree where HEAD left `baseline` where one is given.
fn attempt(root: &Path, baseline: Option<&str>) -> Result<Report, Stop> {
    let files = stop::read_files(root)?;
    let Some(revision) = baseline else {
        return Report::of(&files, None);
    };
    let base = base::read(&GitCommand::default(), root, revision)?;
    let baseline = Baseline {
        revision,
        commit: &base.commit,
        files: &base.files,
    };
    Report::of(&files, Some(baseline))
}

impl Report {
    /// Checks the tree of `files`, against `baseline` where one is given.
    ///
    /// # Errors
    ///
    /// The base's files do not read.
    pub fn of(files: &Files, baseline: Option<Baseline<'_>>) -> Result<Self, Stop> {
        let Some(baseline) = baseline else {
            return Ok(Self {
                checked: check(files),
                compared: None,
            });
        };
        let baselined = check_against(files, baseline.files)
            .map_err(|errors| base::unread(baseline.commit, &errors))?;
        Ok(Self {
            checked: baselined.checked,
            compared: Some(Compared {
                revision: baseline.revision.to_owned(),
                commit: baseline.commit.to_owned(),
                split: baselined.split,
                lint_configs: baselined.lint_configs,
            }),
        })
    }

    /// `failed` where the tree has errors or, under `fail_on`, a counted finding as severe;
    /// `passed` otherwise.
    pub fn outcome(&self, fail_on: Option<Severity>) -> &'static str {
        let marked = self.marked();
        let gated = marked
            .iter()
            .filter(|(_, change)| counts(*change))
            .map(|(finding, _)| finding.severity);
        if self.checked.has_errors() || json::fails_on(fail_on, gated) {
            FAILED
        } else {
            PASSED
        }
    }

    /// The keys of the nodes and decisions the findings, and the findings resolved since the
    /// base, name.
    pub fn subjects(&self) -> BTreeSet<Key> {
        let resolved = self
            .compared
            .iter()
            .flat_map(|compared| &compared.split.resolved);
        self.checked
            .findings()
            .iter()
            .chain(resolved)
            .flat_map(|finding| &finding.subjects)
            .map(|subject| subject.key)
            .collect()
    }

    /// Each finding of the tree with its change against the base, `None` without one.
    fn marked(&self) -> Vec<(&Finding, Option<Change>)> {
        match &self.compared {
            Some(compared) => compared
                .split
                .marked(self.checked.findings())
                .map(|(finding, change)| (finding, Some(change)))
                .collect(),
            None => self.checked.findings().iter().map(|f| (f, None)).collect(),
        }
    }

    fn print(&self) {
        match &self.checked {
            Checked::Unread(errors) => {
                for error in errors {
                    outln!("{error}");
                }
            }
            Checked::Read {
                invalid, config, ..
            } => {
                for error in invalid {
                    outln!("{error}");
                }
                for error in config {
                    outln!("{error}");
                }
                let counted = self
                    .marked()
                    .into_iter()
                    .filter(|(_, change)| counts(*change));
                for (finding, _) in counted {
                    outln!("{finding}");
                }
            }
        }
        if let Some(compared) = &self.compared {
            outln!("{}", summary(compared));
        }
    }

    /// The document `loft check --format json` prints, with `outcome`.
    pub fn document(&self, outcome: &str) -> Value {
        let errors: Vec<Value> = match &self.checked {
            Checked::Unread(errors) => json::read_errors(errors),
            Checked::Read {
                invalid, config, ..
            } => json::validation_errors(invalid)
                .into_iter()
                .chain(config.iter().map(|e| json::read_error("lint-config", e)))
                .collect(),
        };
        let findings: Vec<Value> = self
            .marked()
            .into_iter()
            .map(|(finding, change)| {
                let mut value = json::finding(finding);
                if let Some(change) = change {
                    value["change"] = json!(match change {
                        Change::Introduced => "introduced",
                        Change::Preexisting => "preexisting",
                    });
                }
                value
            })
            .collect();
        let mut fields = Map::new();
        if let Some(compared) = &self.compared {
            let resolved = compared.split.resolved.iter().map(json::finding).collect();
            fields.insert("resolved".to_owned(), Value::Array(resolved));
            fields.insert("baseline".to_owned(), baseline(compared));
        }
        fields.insert("findings".to_owned(), Value::Array(findings));
        json::result(SCHEMA, outcome, Value::Object(fields), errors)
    }
}

/// Whether a finding counts: every finding without a base, the introduced ones against one.
fn counts(change: Option<Change>) -> bool {
    change.is_none_or(|change| change == Change::Introduced)
}

/// The line that ends a check against a base.
fn summary(compared: &Compared) -> String {
    let changes = &compared.split.changes;
    let introduced = changes
        .iter()
        .filter(|change| **change == Change::Introduced)
        .count();
    format!(
        "baseline: {} at {}: {introduced} introduced, {} preexisting, {} resolved",
        compared.revision,
        compared.commit,
        changes.len() - introduced,
        compared.split.resolved.len()
    )
}

fn baseline(compared: &Compared) -> Value {
    let LintConfigs { tree, base, same } = &compared.lint_configs;
    json!({
        "revision": compared.revision,
        "commit": compared.commit,
        "loft": version::VERSION,
        "lintConfig": { "tree": tree, "base": base, "same": same },
    })
}
