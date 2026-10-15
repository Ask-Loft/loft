//! Reading an `OpenSpec` directory into the files the converter takes.

use std::fs;
use std::os::unix::fs::symlink;

use crate::common::Scratch;
use loftspec_store::openspec::{ReadProjectError, read_project};

/// A scratch directory holding one main spec at `specs/notes/spec.md`.
fn project() -> Scratch {
    let scratch = Scratch::new();
    scratch.put("specs/notes/spec.md", "# Notes\n");
    scratch
}

#[test]
fn each_spec_below_specs_is_given_by_its_path_below_specs() {
    let scratch = project();
    scratch.put("specs/billing/invoices/spec.md", "# Invoices\n");
    scratch.put("specs/notes/design.md", "# Not a spec\n");
    let read = read_project(&scratch.0).unwrap();
    let paths: Vec<&str> = read.specs.iter().map(|(path, _)| path.as_str()).collect();
    assert_eq!(paths, ["billing/invoices/spec.md", "notes/spec.md"]);
    assert_eq!(read.specs[1].1, "# Notes\n");
}

#[test]
fn a_spec_reached_through_a_symbolic_link_is_not_given() {
    let scratch = project();
    symlink(
        scratch.0.join("specs/notes"),
        scratch.0.join("specs/linked"),
    )
    .unwrap();
    fs::create_dir_all(scratch.0.join("specs/aliased")).unwrap();
    symlink(
        scratch.0.join("specs/notes/spec.md"),
        scratch.0.join("specs/aliased/spec.md"),
    )
    .unwrap();
    let read = read_project(&scratch.0).unwrap();
    let paths: Vec<&str> = read.specs.iter().map(|(path, _)| path.as_str()).collect();
    assert_eq!(paths, ["notes/spec.md"]);
}

#[test]
fn the_project_description_and_configuration_are_given_when_they_exist() {
    let scratch = project();
    assert_eq!(read_project(&scratch.0).unwrap().project_md, None);
    scratch.put("project.md", "# Project\n");
    scratch.put("config.yaml", "schema: spec-driven\n");
    let read = read_project(&scratch.0).unwrap();
    assert_eq!(read.project_md.as_deref(), Some("# Project\n"));
    assert_eq!(read.config_yaml.as_deref(), Some("schema: spec-driven\n"));
}

#[test]
fn active_changes_are_directories_under_changes_and_archived_ones_count_in_both_places() {
    let scratch = project();
    scratch.put("changes/add-tags/proposal.md", "Why.\n");
    scratch.put("changes/README.md", "Changes.\n");
    scratch.put("changes/archive/2026-01-01-add-notes/proposal.md", "Why.\n");
    scratch.put("changes/archive/.gitkeep", "");
    scratch.put("archive/2025-12-01-add-maps/proposal.md", "Why.\n");
    let read = read_project(&scratch.0).unwrap();
    assert_eq!(read.active_changes, ["add-tags"]);
    assert_eq!(read.archived_changes, 2);
}

#[test]
fn a_directory_that_cannot_be_read_is_an_error_naming_it() {
    let scratch = Scratch::new();
    let absent = scratch.0.join("absent");
    let error = read_project(&absent).unwrap_err();
    assert!(matches!(&error, ReadProjectError::Io { path, .. } if *path == absent));
}

#[test]
fn a_file_that_is_not_utf8_is_an_error_naming_it() {
    for file in ["specs/broken/spec.md", "project.md"] {
        let scratch = project();
        let target = scratch.0.join(file);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, [b'#', b' ', 0xff, b'\n']).unwrap();
        let error = read_project(&scratch.0).unwrap_err();
        assert!(
            matches!(&error, ReadProjectError::NotText(path) if *path == target),
            "{error}"
        );
    }
}
