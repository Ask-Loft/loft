//! How an error or a finding names the sites it concerns before its message: each as
//! `<path>:<line>:<column>`, a site in the file of the site before it by its line and column
//! alone, and a subject without a site by its key.

use crate::common::key;
use loftspec::check::Placed;
use loftspec::codec::{Site, SpecPath};
use loftspec::index::IntegrityError;
use loftspec::lint::{Finding, RuleId, Severity, Subject};
use loftspec::validate::ValidationError;
use loftspec::yaml::{Location, Position};

fn site(path: &str, line: usize, column: usize) -> Site {
    Site {
        path: SpecPath::new(path),
        location: Some(Location::At(Position::new(line, column))),
    }
}

fn finding(subjects: Vec<Subject>) -> Finding {
    Finding {
        rule: RuleId::Lq008,
        severity: Severity::Info,
        subjects,
        term: None,
        body_line: None,
        message: "The names are near-duplicates within their kind.".to_owned(),
    }
}

fn placed(sites: Vec<Site>) -> Placed {
    Placed {
        error: ValidationError::Integrity(IntegrityError::DuplicateKey(key(5))),
        sites,
    }
}

#[test]
fn a_finding_names_once_the_file_that_holds_two_of_its_sites() {
    let subjects = vec![
        Subject {
            key: key(1),
            site: Some(site("a.yaml", 18, 10)),
        },
        Subject {
            key: key(2),
            site: Some(site("a.yaml", 28, 10)),
        },
    ];
    assert_eq!(
        finding(subjects).to_string(),
        "spec/a.yaml:18:10, 28:10: info LQ-008: The names are near-duplicates within their kind."
    );
}

#[test]
fn a_finding_names_each_file_and_a_subject_without_a_site_by_its_key() {
    let subjects = vec![
        Subject {
            key: key(1),
            site: Some(site("a.yaml", 18, 10)),
        },
        Subject {
            key: key(2),
            site: None,
        },
        Subject {
            key: key(3),
            site: Some(site("b.yaml", 4, 6)),
        },
    ];
    assert_eq!(
        finding(subjects).to_string(),
        format!(
            "spec/a.yaml:18:10: {}: spec/b.yaml:4:6: info LQ-008: The names are near-duplicates \
             within their kind.",
            key(2)
        )
    );
}

#[test]
fn a_validation_error_names_once_the_file_that_holds_two_of_its_sites() {
    let error = placed(vec![
        site("a.yaml", 3, 5),
        site("a.yaml", 9, 5),
        site("b.yaml", 1, 6),
    ]);
    assert_eq!(
        error.to_string(),
        format!(
            "spec/a.yaml:3:5, 9:5: spec/b.yaml:1:6: key {} is carried by two elements of the \
             document",
            key(5)
        )
    );
}
