//! The linter over a large converted document: every rule's count, for comparison with the
//! counts its source reported.

use std::collections::BTreeMap;

use crate::common;
use loftspec::codec;
use loftspec::lint::{LintConfig, lint};

#[test]
#[ignore = "reads a local v1 export named by LOFTSPEC_V1_EXPORT; the export is not in this repository"]
fn a_live_v1_export_lints() {
    let document = loftspec::convert::v1::convert(&common::v1::live_export())
        .unwrap()
        .document;
    let files = codec::write(&document);
    let (document, sites) = codec::read_sited(&files).unwrap();
    let started = std::time::Instant::now();
    let findings = lint(&document, &sites, &LintConfig::default());
    let elapsed = started.elapsed();
    let mut counts = BTreeMap::<String, (usize, std::collections::BTreeSet<Vec<_>>)>::new();
    for finding in &findings {
        let entry = counts.entry(finding.rule.to_string()).or_default();
        entry.0 += 1;
        entry
            .1
            .insert(finding.subjects.iter().map(|s| s.key).collect());
    }
    for (rule, (count, subjects)) in &counts {
        println!(
            "{rule}: {count} findings, {} distinct subject sets",
            subjects.len()
        );
    }
    println!("{} findings in {elapsed:?}", findings.len());
}
