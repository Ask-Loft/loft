//! `loft check --baseline` as a function (the `loft check` Feature): the tree's findings split
//! against those of a base tree, both linted under the tree's configuration, so the split
//! measures the change of content alone. A finding is the same at both sides when its
//! [`Identity`] is. Findings of one identity pair as a multiset, those at the same body line
//! first, then the rest in report order.

use std::collections::BTreeMap;

use crate::canonical::digest_bytes;
use crate::codec::{Files, LINT, ReadError, SpecPath, read_sited};
use crate::lint::{Finding, Identity, lint};

use super::{Checked, checked_under, lint_config};

/// Whether the tree introduced a finding or kept it from the base.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Introduced,
    Preexisting,
}

/// The tree's findings split against the base's.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Split {
    /// The change of each finding of the tree, in the order of its findings.
    pub changes: Vec<Change>,
    /// The base's findings the tree no longer holds, as the base reports them.
    pub resolved: Vec<Finding>,
}

impl Split {
    /// Each of `findings`, the tree's findings this split was made of, with its change.
    pub fn marked<'a>(
        &'a self,
        findings: &'a [Finding],
    ) -> impl Iterator<Item = (&'a Finding, Change)> + 'a {
        debug_assert_eq!(findings.len(), self.changes.len(), "the tree's findings");
        findings.iter().zip(self.changes.iter().copied())
    }
}

/// The two trees' `spec/lint.yaml`: the SHA-256 digest of each file's bytes, `None` for one that
/// is absent, and whether both give the same configuration once read, which a change of
/// quoting alone leaves true.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintConfigs {
    pub tree: Option<String>,
    pub base: Option<String>,
    pub same: bool,
}

/// The tree checked, with its findings split against a base tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Baselined {
    pub checked: Checked,
    /// The split, empty where the tree does not read or its lint configuration does not.
    pub split: Split,
    pub lint_configs: LintConfigs,
}

/// Checks the tree `files`, and splits its findings against those of `base` linted under the
/// tree's configuration. The base's validation errors are not reported.
///
/// # Errors
///
/// The base tree does not read: its read errors.
pub fn check_against(files: &Files, base: &Files) -> Result<Baselined, Vec<ReadError>> {
    let (before, before_sites) = read_sited(base)?;
    let config = lint_config(files);
    let same = matches!(
        (&config, &lint_config(base)),
        (Ok(now), Ok(then)) if now == then
    );
    let (checked, used) = checked_under(files, config);
    let split = used.map_or_else(Split::default, |config| {
        split(checked.findings(), &lint(&before, &before_sites, &config))
    });
    Ok(Baselined {
        checked,
        split,
        lint_configs: LintConfigs {
            tree: digest(files),
            base: digest(base),
            same,
        },
    })
}

/// The SHA-256 digest of a tree's `spec/lint.yaml`, where it has one.
fn digest(files: &Files) -> Option<String> {
    files
        .get(&SpecPath::new(LINT))
        .map(|bytes| format!("sha256:{}", digest_bytes(bytes)))
}

/// Splits the tree's findings against the base's: a tree finding paired with a base finding of
/// its identity is preexisting, one left over introduced, and a base finding left over resolved.
#[must_use]
pub fn split(tree: &[Finding], base: &[Finding]) -> Split {
    let mut unpaired: BTreeMap<Identity, Vec<usize>> = BTreeMap::new();
    for (index, finding) in base.iter().enumerate() {
        unpaired.entry(finding.identity()).or_default().push(index);
    }
    let identities: Vec<Identity> = tree.iter().map(Finding::identity).collect();
    let mut paired = vec![false; tree.len()];
    let mut take = |same_line: bool| {
        for (index, finding) in tree.iter().enumerate() {
            if paired[index] {
                continue;
            }
            let Some(candidates) = unpaired.get_mut(&identities[index]) else {
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
