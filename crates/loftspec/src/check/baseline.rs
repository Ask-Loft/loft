//! `loft check --baseline` as a function (the `loft check` Feature): the tree's findings split
//! against those of a base tree, both linted under the tree's configuration, so the split
//! measures the change of content alone. A finding is the same at both sides when its rule, its
//! subject keys and its term are; its site and its message are left out, since a moved node
//! changes the one and a count the other. Findings of one identity pair as a multiset, those at
//! the same body line first, then the rest in report order.

use std::collections::BTreeMap;

use crate::codec::{Files, ReadError, read_sited};
use crate::identity::Key;
use crate::lint::{Finding, RuleId, lint};

use super::{Checked, checked_under, lint_config};

/// Whether the tree introduced a finding or kept it from the base.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Introduced,
    Preexisting,
}

/// The tree's findings split against the base's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Split {
    /// The change of each finding of the tree, in the order of its findings.
    pub changes: Vec<Change>,
    /// The base's findings the tree no longer holds, as the base reports them.
    pub resolved: Vec<Finding>,
}

/// The tree checked, with its findings split against a base tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Baselined {
    pub checked: Checked,
    /// The split, where the tree reads and its lint configuration does.
    pub split: Option<Split>,
    /// Whether the base's `spec/lint.yaml` gives the configuration the tree's gives.
    pub same_config: bool,
}

/// Checks the tree `files`, and splits its findings against those of `base` linted under the
/// tree's configuration. The base's validation errors are not reported.
///
/// # Errors
///
/// The base tree does not read: its read errors.
pub fn check_against(files: &Files, base: &Files) -> Result<Baselined, Vec<ReadError>> {
    let (before, before_sites) = read_sited(base)?;
    let (checked, config) = checked_under(files);
    let split = config.as_ref().map(|config| {
        let then = lint(&before, &before_sites, config);
        split(checked.findings(), &then)
    });
    let same_config = matches!(
        (lint_config(files), lint_config(base)),
        (Ok(now), Ok(then)) if now == then
    );
    Ok(Baselined {
        checked,
        split,
        same_config,
    })
}

/// What makes two findings the same at both sides.
type Identity = (RuleId, Vec<Key>, Option<String>);

fn identity(finding: &Finding) -> Identity {
    let mut keys: Vec<Key> = finding.subjects.iter().map(|subject| subject.key).collect();
    keys.sort_unstable();
    (finding.rule, keys, finding.term.clone())
}

/// Splits the tree's findings against the base's: a tree finding paired with a base finding of
/// its identity is preexisting, one left over introduced, and a base finding left over resolved.
#[must_use]
pub fn split(tree: &[Finding], base: &[Finding]) -> Split {
    let mut unpaired: BTreeMap<Identity, Vec<usize>> = BTreeMap::new();
    for (index, finding) in base.iter().enumerate() {
        unpaired.entry(identity(finding)).or_default().push(index);
    }
    let mut paired = vec![false; tree.len()];
    let mut take = |same_line: bool| {
        for (index, finding) in tree.iter().enumerate() {
            if paired[index] {
                continue;
            }
            let Some(candidates) = unpaired.get_mut(&identity(finding)) else {
                continue;
            };
            let at = candidates
                .iter()
                .position(|then| !same_line || base[*then].body_line == finding.body_line);
            if let Some(at) = at {
                candidates.remove(at);
                paired[index] = true;
            }
        }
    };
    take(true);
    take(false);
    let mut resolved: Vec<usize> = unpaired.into_values().flatten().collect();
    resolved.sort_unstable();
    Split {
        changes: paired
            .into_iter()
            .map(|paired| {
                if paired {
                    Change::Preexisting
                } else {
                    Change::Introduced
                }
            })
            .collect(),
        resolved: resolved
            .into_iter()
            .map(|index| base[index].clone())
            .collect(),
    }
}
