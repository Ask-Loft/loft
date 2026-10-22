//! `loft check` (F-38): the reader, then the validator, then the linter, with an exit code a CI
//! job can gate on. In text, each error and finding is one line, errors before findings (AC-522,
//! AC-595); with `--format json`, one document on standard output (RQ-155). With `--baseline
//! <revision>`, the findings are marked against the tree where HEAD left the revision: the text
//! prints the introduced ones and a count, and `--fail-on` gates on the introduced ones alone.

use std::path::Path;
use std::process::ExitCode;

use loftspec::check::{Change, Checked, LintConfigs, Split, check, check_against};
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

/// What `loft check` was asked for.
pub struct Options<'a> {
    pub fail_on: Option<Severity>,
    pub baseline: Option<&'a str>,
}

/// The tree's findings split against a base, and where the base came from.
struct Compared {
    revision: String,
    commit: String,
    split: Split,
    lint_configs: LintConfigs,
}

/// Each finding of the tree with its change against the base, `None` without one.
fn marked<'a>(
    checked: &'a Checked,
    compared: Option<&'a Compared>,
) -> Vec<(&'a Finding, Option<Change>)> {
    match compared {
        Some(compared) => compared
            .split
            .marked(checked.findings())
            .map(|(finding, change)| (finding, Some(change)))
            .collect(),
        None => checked.findings().iter().map(|f| (f, None)).collect(),
    }
}

/// Whether a finding counts: every finding without a base, the introduced ones against one.
fn counts(change: Option<Change>) -> bool {
    change.is_none_or(|change| change == Change::Introduced)
}

pub fn run(root: &Path, options: &Options, as_json: bool) -> Finished {
    let (checked, compared) = match attempt(root, options.baseline) {
        Ok(result) => result,
        Err(stop) => return stop.exit(SCHEMA, as_json),
    };
    let marked = marked(&checked, compared.as_ref());
    let gated = marked
        .iter()
        .filter(|(_, change)| counts(*change))
        .map(|(finding, _)| finding.severity);
    let failed = checked.has_errors() || json::fails_on(options.fail_on, gated);
    let outcome = if failed { "failed" } else { "passed" };
    let document = document(&checked, &marked, compared.as_ref(), outcome);
    if as_json {
        json::print(&document);
    } else {
        print(&checked, &marked, compared.as_ref());
    }
    let code = if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    };
    Finished::with(code, outcome, json::errors_of(&document))
}

/// The document `loft check --format json` prints for the tree under `root`, against the tree
/// where HEAD left `baseline` where one is given, with no `--fail-on`, as a `loft` view of Yard
/// Log records it.
pub fn json_document(root: &Path, baseline: Option<&str>) -> Result<Value, Stop> {
    let (checked, compared) = attempt(root, baseline)?;
    let marked = marked(&checked, compared.as_ref());
    let outcome = if checked.has_errors() {
        "failed"
    } else {
        "passed"
    };
    Ok(document(&checked, &marked, compared.as_ref(), outcome))
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
        commit: base.commit,
        split: baselined.split,
        lint_configs: baselined.lint_configs,
    };
    Ok((baselined.checked, Some(compared)))
}

fn print(checked: &Checked, marked: &[(&Finding, Option<Change>)], compared: Option<&Compared>) {
    match checked {
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
            for (finding, change) in marked {
                if counts(*change) {
                    outln!("{finding}");
                }
            }
        }
    }
    if let Some(compared) = compared {
        outln!("{}", summary(compared));
    }
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

fn document(
    checked: &Checked,
    marked: &[(&Finding, Option<Change>)],
    compared: Option<&Compared>,
    outcome: &str,
) -> Value {
    let errors: Vec<Value> = match checked {
        Checked::Unread(errors) => json::read_errors(errors),
        Checked::Read {
            invalid, config, ..
        } => json::validation_errors(invalid)
            .into_iter()
            .chain(config.iter().map(|e| json::read_error("lint-config", e)))
            .collect(),
    };
    let findings: Vec<Value> = marked
        .iter()
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
    if let Some(compared) = compared {
        let resolved = compared.split.resolved.iter().map(json::finding).collect();
        fields.insert("resolved".to_owned(), Value::Array(resolved));
        fields.insert("baseline".to_owned(), baseline(compared));
    }
    fields.insert("findings".to_owned(), Value::Array(findings));
    json::result(SCHEMA, outcome, Value::Object(fields), errors)
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
