//! `loft check --baseline` as a function (the `loft check` Feature): the findings of a tree split
//! into those it introduced and those it kept against a base tree linted under the tree's own
//! configuration, and the base's findings it resolved.

use crate::common::key;
use crate::common::text::{FEATURE, edited, files, path};
use loftspec::check::{Change, check_against, split};
use loftspec::codec::{Files, LINT, Site};
use loftspec::lint::{Finding, RuleId, Severity, Subject};

/// A finding of `rule` naming the keys `subjects`, about `term`, on body line `body_line`.
fn finding(
    rule: RuleId,
    subjects: &[u32],
    term: Option<&str>,
    body_line: Option<usize>,
) -> Finding {
    Finding {
        rule,
        severity: Severity::Warning,
        subjects: subjects
            .iter()
            .map(|n| Subject {
                key: key(*n),
                site: None,
            })
            .collect(),
        term: term.map(str::to_owned),
        body_line,
        message: "The text contains a vague quantifier.".to_owned(),
    }
}

fn vague(body_line: usize) -> Finding {
    finding(RuleId::Lq002, &[3], Some("some"), Some(body_line))
}

#[test]
fn a_finding_of_the_same_rule_subjects_and_term_is_preexisting() {
    let split = split(&[vague(1)], &[vague(1)]);
    assert_eq!(split.changes, [Change::Preexisting]);
    assert_eq!(split.resolved, []);
}

#[test]
fn a_finding_whose_node_lies_at_another_file_or_line_is_preexisting() {
    let mut moved = vague(1);
    moved.subjects[0].site = Some(Site {
        path: path(FEATURE),
        location: None,
    });
    let split = split(&[moved], &[vague(1)]);
    assert_eq!(split.changes, [Change::Preexisting]);
}

#[test]
fn a_finding_whose_message_gives_another_count_is_preexisting() {
    let mut now = finding(RuleId::V2006, &[4], None, None);
    now.message = "The Requirement holds 11 criteria.".to_owned();
    let mut then = now.clone();
    then.message = "The Requirement holds 9 criteria.".to_owned();
    assert_eq!(split(&[now], &[then]).changes, [Change::Preexisting]);
}

#[test]
fn a_finding_of_another_rule_subject_or_term_is_introduced_and_the_base_s_resolved() {
    let base = vague(1);
    for other in [
        finding(RuleId::Lq003, &[3], Some("some"), Some(1)),
        finding(RuleId::Lq002, &[4], Some("some"), Some(1)),
        finding(RuleId::Lq002, &[3], Some("several"), Some(1)),
    ] {
        let split = split(std::slice::from_ref(&other), std::slice::from_ref(&base));
        assert_eq!(split.changes, [Change::Introduced], "{other:?}");
        assert_eq!(split.resolved, std::slice::from_ref(&base), "{other:?}");
    }
}

#[test]
fn the_order_of_a_finding_s_subjects_does_not_tell_it_apart() {
    let now = finding(RuleId::Lq008, &[3, 4], None, None);
    let then = finding(RuleId::Lq008, &[4, 3], None, None);
    assert_eq!(split(&[now], &[then]).changes, [Change::Preexisting]);
}

#[test]
fn two_findings_where_the_base_had_one_give_one_introduced() {
    let split = split(&[vague(1), vague(1)], &[vague(1)]);
    assert_eq!(split.changes, [Change::Preexisting, Change::Introduced]);
    assert_eq!(split.resolved, []);
}

#[test]
fn repeated_findings_pair_with_the_base_s_at_the_same_body_line_first() {
    let one = split(&[vague(1), vague(5)], &[vague(5)]);
    assert_eq!(one.changes, [Change::Introduced, Change::Preexisting]);
    let two = split(&[vague(2), vague(7)], &[vague(9), vague(7)]);
    assert_eq!(two.changes, [Change::Preexisting, Change::Preexisting]);
}

#[test]
fn a_finding_only_the_base_holds_is_resolved_as_the_base_reports_it() {
    let mut then = vague(3);
    then.subjects[0].site = Some(Site {
        path: path(FEATURE),
        location: None,
    });
    let split = split(&[], std::slice::from_ref(&then));
    assert_eq!(split.changes, []);
    assert_eq!(split.resolved, [then]);
}

/// The small document's files with its Requirement shall `shall`, and `lint.yaml` holding
/// `lint`.
fn tree(shall: &str, lint: Option<&str>) -> Files {
    let mut files = edited(FEATURE, PLAIN, shall);
    if let Some(lint) = lint {
        files.insert(path(LINT), lint.as_bytes().to_vec());
    }
    files
}

const PLAIN: &str = "write a key on every node.";
const VAGUE: &str = "write a key on some nodes.";
const NO_LQ_002: &str = "rules:\n  - rule: LQ-002\n    enabled: false\n";

fn lq_002(changes: &[Change], findings: &[Finding]) -> Vec<Change> {
    findings
        .iter()
        .zip(changes)
        .filter(|(finding, _)| finding.rule == RuleId::Lq002)
        .map(|(_, change)| *change)
        .collect()
}

#[test]
fn a_tree_checked_against_itself_holds_no_introduced_and_no_resolved_finding() {
    let baselined = check_against(&files(), &files()).unwrap();
    let split = baselined.split;
    assert_ne!(baselined.checked.findings(), []);
    assert!(
        split
            .changes
            .iter()
            .all(|change| *change == Change::Preexisting)
    );
    assert_eq!(split.resolved, []);
}

#[test]
fn a_vague_quantifier_added_to_a_statement_is_introduced() {
    let baselined = check_against(&tree(VAGUE, None), &tree(PLAIN, None)).unwrap();
    let split = baselined.split;
    assert_eq!(
        lq_002(&split.changes, baselined.checked.findings()),
        [Change::Introduced]
    );
}

#[test]
fn the_base_is_linted_under_the_working_tree_s_configuration() {
    let baselined = check_against(&tree(VAGUE, None), &tree(VAGUE, Some(NO_LQ_002))).unwrap();
    let split = baselined.split;
    assert_eq!(
        lq_002(&split.changes, baselined.checked.findings()),
        [Change::Preexisting]
    );
    let quiet = check_against(&tree(VAGUE, Some(NO_LQ_002)), &tree(VAGUE, None)).unwrap();
    assert_eq!(quiet.split.resolved, []);
}

#[test]
fn each_configuration_file_is_digested_by_its_bytes_and_an_absent_one_is_none() {
    let quoted = NO_LQ_002.replace("LQ-002", "\"LQ-002\"");
    let configs = check_against(
        &tree(PLAIN, Some(NO_LQ_002)),
        &tree(PLAIN, Some(quoted.as_str())),
    )
    .unwrap()
    .lint_configs;
    assert!(configs.same, "{configs:?}");
    assert_ne!(configs.tree, configs.base);
    assert!(
        configs
            .tree
            .as_deref()
            .is_some_and(|d| d.starts_with("sha256:"))
    );
    let absent = check_against(&tree(PLAIN, None), &tree(PLAIN, None)).unwrap();
    assert_eq!(
        (absent.lint_configs.tree, absent.lint_configs.base),
        (None, None)
    );
}

#[test]
fn the_configurations_are_the_same_exactly_when_both_files_give_one() {
    let quoted = NO_LQ_002.replace("LQ-002", "\"LQ-002\"");
    let same = |tree_lint, base_lint| {
        check_against(&tree(PLAIN, tree_lint), &tree(PLAIN, base_lint))
            .unwrap()
            .lint_configs
            .same
    };
    assert!(same(None, None));
    assert!(same(Some(NO_LQ_002), Some(NO_LQ_002)));
    assert!(same(Some(NO_LQ_002), Some(quoted.as_str())));
    assert!(!same(Some(NO_LQ_002), None));
    assert!(!same(
        None,
        Some("rules:\n  - rule: XX-999\n    enabled: false\n")
    ));
}

#[test]
fn the_base_tree_s_validation_errors_are_not_reported() {
    let mut base = files();
    let product = path(crate::common::text::PRODUCT);
    let text = String::from_utf8(base[&product].clone()).unwrap();
    let doubled = text.replacen("  - portable\n", "  - portable\n  - portable\n", 1);
    assert_ne!(doubled, text);
    base.insert(product, doubled.into_bytes());
    assert!(loftspec::check::check(&base).has_errors());
    let baselined = check_against(&files(), &base).unwrap();
    assert!(!baselined.checked.has_errors());
}

#[test]
fn a_base_tree_that_does_not_read_is_an_error() {
    let mut base = files();
    base.insert(path(FEATURE), b"name: [\n".to_vec());
    let errors = check_against(&files(), &base).unwrap_err();
    assert_ne!(errors, []);
}
