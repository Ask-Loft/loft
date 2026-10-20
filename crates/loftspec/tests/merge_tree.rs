//! The tree merger (F-32): base, ours and theirs merged as three whole `spec/` trees, each record
//! paired by its key whatever file holds it on each side, so that a record moved to another file
//! on one side and changed on the other comes out once.

use crate::common::merge_tree::{
    LABELS_FEATURE, carried, driver, dump, edited, features, file, merged, merged_files, moved,
    read, requirement_4, requirements, three_features, two_features, with_lint,
};
use crate::common::text::{FEATURE, PRODUCT};
use crate::common::{feature, key, label, requirement, slug, small_document};
use loftspec::codec::{self, Files, LABELS, LINT, MANIFEST, Problem, SpecPath};
use loftspec::fmt;
use loftspec::ids::FeatureId;
use loftspec::merge::{MergeError, TreeMergeError, merge_tree};
use loftspec::statement::{EarsForm, Statement};

const EDIT: &str = "Changed where it was.";

#[test]
fn a_record_moved_to_another_file_and_changed_where_it_was_comes_out_once_with_the_change() {
    let base = two_features();
    let merged = merged(&base, &moved(&base), &edited(&base, EDIT));
    assert_eq!(carried(&merged, 4), 1, "{}", file(&merged, LABELS_FEATURE));
    let mut document = read(&merged);
    assert_eq!(requirements(&mut document, 0).len(), 0);
    let labels = requirements(&mut document, 1);
    assert_eq!(labels.len(), 2);
    assert_eq!(labels[1].details.body.as_deref(), Some(EDIT));
    assert!(!file(&merged, FEATURE).contains("conflicts:"));
    assert!(!file(&merged, LABELS_FEATURE).contains("conflicts:"));
}

#[test]
fn a_record_both_sides_moved_to_one_place_is_there_without_a_conflict_over_its_place() {
    let base = two_features();
    let ours = moved(&base);
    let merged = merged(&base, &ours, &edited(&ours, EDIT));
    assert_eq!(merged.conflicts, 0, "{}", dump(&merged));
    let mut document = read(&merged);
    let labels = requirements(&mut document, 1);
    assert_eq!(labels[1].details.body.as_deref(), Some(EDIT));
}

#[test]
fn a_feature_whose_slug_one_side_changed_is_written_under_its_new_name_and_leaves_its_old_file() {
    let base = two_features();
    let mut ours = moved(&base);
    features(&mut ours)[1].slug = slug("labelling");
    let merged = merged(&base, &ours, &edited(&base, EDIT));
    let renamed = "products/example-product/identity/labelling.yaml";
    assert!(file(&merged, renamed).contains(EDIT), "{}", dump(&merged));
    assert!(!merged.files.contains_key(&SpecPath::new(LABELS_FEATURE)));
    assert_eq!(carried(&merged, 4), 1);
    read(&merged);
}

#[test]
fn a_feature_one_side_moved_to_another_group_is_written_in_that_group_s_directory() {
    let mut base = two_features();
    let mut naming = base.products[0].feature_groups[0].clone();
    naming.id = Some(loftspec::ids::FeatureGroupId::new(key(45)));
    naming.slug = slug("naming");
    naming.name = "Naming".to_owned();
    naming.features = Vec::new();
    naming.constraints = Vec::new();
    base.products[0].feature_groups.push(naming);
    let mut ours = base.clone();
    let labels = features(&mut ours).remove(1);
    ours.products[0].feature_groups[1].features.push(labels);
    let mut theirs = base.clone();
    requirements(&mut theirs, 1)[0].details.body = Some(EDIT.to_owned());
    let merged = merged(&base, &ours, &theirs);
    let moved = "products/example-product/naming/labels.yaml";
    assert!(file(&merged, moved).contains(EDIT), "{}", dump(&merged));
    assert!(!merged.files.contains_key(&SpecPath::new(LABELS_FEATURE)));
    let document = read(&merged);
    let groups = &document.products[0].feature_groups;
    assert_eq!(groups[0].features.len(), 1);
    assert_eq!(groups[1].features[0].id, Some(FeatureId::new(key(40))));
}

#[test]
fn a_file_that_holds_no_record_of_the_result_is_absent_from_it() {
    let base = two_features();
    let mut ours = base.clone();
    features(&mut ours).remove(1);
    let merged = merged(&base, &ours, &base);
    assert!(!merged.files.contains_key(&SpecPath::new(LABELS_FEATURE)));
    assert_eq!(carried(&merged, 40), 0);
    read(&merged);
}

#[test]
fn an_empty_base_tree_merges_as_a_base_that_holds_no_record() {
    let document = small_document();
    let tree = codec::write(&document);
    let merged = merged_files(&Files::new(), &tree, &tree);
    assert_eq!(merged.files, tree);
    let mut theirs = document.clone();
    features(&mut theirs)[0].details.body = Some("Keys name nodes.".to_owned());
    let merged = merged_files(&Files::new(), &tree, &codec::write(&theirs));
    let text = file(&merged, FEATURE);
    assert!(text.contains("body: \"Keys address nodes.\""), "{text}");
    assert!(text.contains("  - field: body\n    ours:"), "{text}");
    assert_eq!(merged.conflicts, 1);
}

#[test]
fn the_fields_of_a_moved_record_merge_as_the_merge_driver_merges_them() {
    let base = two_features();
    let ours = edited(&moved(&base), EDIT);
    let mut theirs = edited(&base, EDIT);
    requirement_4(&mut theirs).statement = Statement::Form(EarsForm::Ubiquitous {
        system: "The writer".to_owned(),
        shall: "write one key on every node.".to_owned(),
    });
    let merged = merged(&base, &ours, &theirs);
    assert_eq!(merged.conflicts, 0, "{}", dump(&merged));
    let text = file(&merged, LABELS_FEATURE);
    assert_eq!(text.matches(EDIT).count(), 1, "{text}");
    assert!(
        text.contains("shall: \"write one key on every node.\""),
        "{text}"
    );
}

#[test]
fn a_conflict_on_a_moved_record_is_the_typed_record_the_merge_driver_writes() {
    let base = two_features();
    let ours = edited(&base, "Ours.");
    let theirs = edited(&base, "Theirs.");
    let in_place = driver(
        FEATURE,
        &codec::write(&base),
        &codec::write(&ours),
        &codec::write(&theirs),
    );
    let in_place = String::from_utf8(in_place).unwrap();
    let conflict = "    conflicts:
      - field: body
        ours: \"Ours.\"
        theirs: \"Theirs.\"
";
    assert!(in_place.contains(conflict), "{in_place}");
    let merged = merged(&base, &edited(&moved(&base), "Ours."), &theirs);
    assert!(
        file(&merged, LABELS_FEATURE).ends_with(conflict),
        "{}",
        dump(&merged)
    );
    assert_eq!(merged.conflicts, 1);
}

#[test]
fn an_ordered_list_merges_as_the_merge_driver_merges_it() {
    let base = two_features();
    let mut ours = base.clone();
    requirements(&mut ours, 1).insert(0, requirement(key(50), "Ours shall add.", Vec::new()));
    let mut theirs = base.clone();
    requirements(&mut theirs, 1).push(requirement(key(51), "Theirs shall add.", Vec::new()));
    let [base, ours, theirs] = [&base, &ours, &theirs].map(codec::write);
    let merged = merged_files(&base, &ours, &theirs);
    let expected = driver(LABELS_FEATURE, &base, &ours, &theirs);
    assert_eq!(merged.files[&SpecPath::new(LABELS_FEATURE)], expected);
    let text = file(&merged, LABELS_FEATURE);
    let at = |key_text: &str| text.find(key_text).unwrap();
    assert!(at("00000000002a") < at("000000000033"), "{text}");
    assert!(at("000000000032") < at("000000000029"), "{text}");
}

#[test]
fn where_no_record_changes_file_each_file_holds_the_bytes_the_merge_driver_writes() {
    let base = three_features();
    let mut ours = edited(&base, "Ours.");
    features(&mut ours)[2].details.body = Some("Aliases name nodes.".to_owned());
    ours.products[0].details.body = Some("Ours' example.".to_owned());
    let mut theirs = edited(&base, "Theirs.");
    requirements(&mut theirs, 1).push(requirement(key(51), "Theirs shall add.", Vec::new()));
    features(&mut theirs).push(feature(key(47), "Prefixes", Vec::new()));
    let [base, ours, theirs] = [&base, &ours, &theirs].map(codec::write);
    let merged = merged_files(&base, &ours, &theirs);
    for path in ours.keys().chain(theirs.keys()) {
        let expected = match (ours.get(path), theirs.get(path)) {
            (Some(_), Some(_)) => driver(path.as_str(), &base, &ours, &theirs),
            (Some(added), None) | (None, Some(added)) => added.clone(),
            (None, None) => unreachable!("a path a side holds"),
        };
        assert_eq!(
            String::from_utf8_lossy(&merged.files[path]),
            String::from_utf8_lossy(&expected),
            "{path}"
        );
    }
    assert_eq!(merged.files.len(), ours.len() + 1);
    assert_eq!(merged.conflicts, 1);
}

#[test]
fn the_manifest_of_the_result_is_the_file_the_merge_driver_writes_for_the_three_manifests() {
    let base = two_features();
    let mut ours = base.clone();
    ours.publish_branch = Some("main".to_owned());
    let mut theirs = base.clone();
    theirs
        .products
        .push(crate::common::product(key(60), "Second Product"));
    let [base, ours, theirs] = [&base, &ours, &theirs].map(codec::write);
    let merged = merged_files(&base, &ours, &theirs);
    let expected = driver(MANIFEST, &base, &ours, &theirs);
    assert_eq!(merged.files[&SpecPath::new(MANIFEST)], expected);
    let document = read(&merged);
    assert_eq!(document.publish_branch.as_deref(), Some("main"));
    assert_eq!(document.products.len(), 2);
}

#[test]
fn the_bindings_of_the_result_are_the_file_the_merge_driver_writes_for_the_three_label_files() {
    let base = three_features();
    let mut ours = base.clone();
    ours.bindings
        .bind(FeatureId::new(key(40)), label("F-2"))
        .unwrap();
    let mut theirs = base.clone();
    theirs
        .bindings
        .bind(FeatureId::new(key(46)), label("F-2"))
        .unwrap();
    let [base, ours, theirs] = [&base, &ours, &theirs].map(codec::write);
    let merged = merged_files(&base, &ours, &theirs);
    let expected = merge_label_files(&base, &ours, &theirs);
    assert_eq!(merged.files[&SpecPath::new(LABELS)], expected.bytes);
    assert_eq!(merged.disputed, expected.disputed);
    assert_eq!(merged.disputed, [label("F-2")]);
}

fn merge_label_files(base: &Files, ours: &Files, theirs: &Files) -> loftspec::merge::Merged {
    let path = SpecPath::new(LABELS);
    loftspec::merge::merge_file(&path, &base[&path], &ours[&path], &theirs[&path]).unwrap()
}

const LINT_BASE: &str = "rules:\n  - rule: SH-002\n    enabled: false\n";
const LINT_OURS: &str = "rules:\n  - rule: SH-003\n    enabled: false\n";
const LINT_THEIRS: &str = "rules:\n  - rule: LQ-007\n    enabled: false\n";

/// The lint configuration of the merge of the small document's tree with these three.
fn merged_lint(base: &str, ours: &str, theirs: &str) -> Result<Vec<u8>, TreeMergeError> {
    let document = small_document();
    let [base, ours, theirs] = [base, ours, theirs].map(|text| with_lint(&document, text));
    let merged = merge_tree(&base, &ours, &theirs)?;
    Ok(merged.files[&SpecPath::new(LINT)].clone())
}

#[test]
fn a_lint_configuration_one_side_changed_is_that_side_s_file_in_the_result() {
    assert_eq!(
        merged_lint(LINT_BASE, LINT_OURS, LINT_BASE).unwrap(),
        LINT_OURS.as_bytes()
    );
    assert_eq!(
        merged_lint(LINT_BASE, LINT_BASE, LINT_THEIRS).unwrap(),
        LINT_THEIRS.as_bytes()
    );
}

#[test]
fn a_lint_configuration_both_sides_changed_alike_is_those_contents_in_the_result() {
    assert_eq!(
        merged_lint(LINT_BASE, LINT_OURS, LINT_OURS).unwrap(),
        LINT_OURS.as_bytes()
    );
}

#[test]
fn a_lint_configuration_both_sides_changed_apart_refuses_the_merge_naming_the_file() {
    let refused = merged_lint(LINT_BASE, LINT_OURS, LINT_THEIRS).unwrap_err();
    assert_eq!(
        refused,
        TreeMergeError::File {
            path: SpecPath::new(LINT),
            error: MergeError::BothChanged,
        }
    );
    assert!(refused.to_string().contains("spec/lint.yaml"), "{refused}");
}

/// The merges of moves that the merge driver alone would leave twice in the tree.
fn moved_merges() -> Vec<loftspec::merge::MergedTree> {
    let base = three_features();
    let mut apart = base.clone();
    let moving = requirements(&mut apart, 0).remove(0);
    requirements(&mut apart, 2).push(moving);
    let mut deleted = base.clone();
    requirements(&mut deleted, 1).clear();
    let mut ours = base.clone();
    let moving = requirements(&mut ours, 1).remove(0);
    requirements(&mut ours, 0).push(moving);
    vec![
        merged(&base, &moved(&base), &edited(&base, EDIT)),
        merged(&base, &moved(&base), &apart),
        merged(&base, &ours, &deleted),
    ]
}

#[test]
fn the_result_reads_without_a_record_moved_in_one_branch_and_changed_in_the_other() {
    for merged in moved_merges() {
        let read = codec::read(&merged.files);
        let errors = read.as_ref().err().into_iter().flatten();
        let moved = errors.filter(|error| matches!(error.problem, Problem::MovedAndChanged(_)));
        assert_eq!(moved.count(), 0, "{}", dump(&merged));
        assert!(read.is_ok(), "{read:?}");
    }
}

#[test]
fn the_result_reads_without_a_key_carried_twice() {
    for merged in moved_merges() {
        let read = codec::read(&merged.files);
        let errors = read.as_ref().err().into_iter().flatten();
        let twice = errors.filter(|error| matches!(error.problem, Problem::DuplicateKey(_)));
        assert_eq!(twice.count(), 0, "{}", dump(&merged));
        assert!(read.is_ok(), "{read:?}");
        for n in [3, 4, 5, 40, 41, 46] {
            assert!(carried(&merged, n) <= 1, "{n} in {}", dump(&merged));
        }
    }
}

#[test]
fn each_file_of_the_result_is_in_the_formatter_s_canonical_layout() {
    for merged in moved_merges() {
        let formatted = fmt::format(&merged.files).expect("the result formats");
        assert!(
            formatted.changed.is_empty(),
            "{:?}",
            formatted.changed.keys()
        );
    }
    let product = file(&moved_merges()[0], PRODUCT);
    assert!(product.contains("features:"), "{product}");
}
