//! The files of Yard Log: a root and one file for each stage, named by the stage's key, under
//! `.yard/<branch>/`, written in one canonical layout, and refused to every write while they do
//! not read.

use std::fs;

use crate::common::{Failed, Tree, read, try_write, write};
use serde_json::json;
use yardlog::codec::stage_file;
use yardlog_store::YardStoreError;

fn planned(tree: &Tree) -> yardlog_store::YardStore {
    let store = tree.store("main");
    write(
        &store,
        &json!([
            {"id": "design", "kind": "stage", "title": "Design"},
            {"id": "build", "kind": "stage", "title": "Build"},
            {"id": "intro", "kind": "note", "stage": "design", "markdown": "Hello."},
            {"id": "steps", "kind": "note", "stage": "build", "markdown": "Steps."}
        ]),
    );
    store
}

#[test]
fn yard_log_is_a_root_and_one_file_for_each_stage_named_by_its_key() {
    let tree = Tree::new();
    let store = planned(&tree);
    let log = read(&store);
    let folder = tree.top().join(".yard/main");
    let root = fs::read_to_string(folder.join("yardlog.yaml")).unwrap();
    assert!(root.starts_with("format: yardlog/1\n"), "{root}");
    assert!(root.contains(&log.root.key.to_string()), "{root}");
    let mut names: Vec<String> = fs::read_dir(&folder)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    let mut expected: Vec<String> = log
        .stages
        .iter()
        .map(|file| stage_file(file.stage.key.stage()))
        .chain(["yardlog.yaml".to_owned()])
        .collect();
    expected.sort();
    assert_eq!(names, expected);
    let design =
        fs::read_to_string(folder.join(stage_file(log.stages[0].stage.key.stage()))).unwrap();
    assert!(design.contains("Hello."), "{design}");
    assert!(!root.contains("Hello."), "{root}");
}

#[test]
fn a_stage_keeps_its_file_when_its_id_changes() {
    let tree = Tree::new();
    let store = planned(&tree);
    let key = read(&store).stages[0].stage.key.stage();
    store
        .update(|log| -> Result<_, Failed> {
            let mut log = log.unwrap();
            let id = yardlog::Handle::parse("design-2").unwrap();
            log.root.stages[0].id = id.clone();
            log.stages[0].stage.id = id;
            Ok((log, ()))
        })
        .unwrap();
    let folder = tree.top().join(".yard/main");
    assert!(folder.join(stage_file(key)).is_file());
    assert_eq!(read(&store).stages[0].stage.id.as_str(), "design-2");
}

#[test]
fn every_version_of_a_block_is_kept_in_its_stage_s_file() {
    let tree = Tree::new();
    let store = planned(&tree);
    write(
        &store,
        &json!([{"id": "intro", "kind": "note", "stage": "design", "markdown": "Hello again."}]),
    );
    let log = read(&store);
    let intro = log.find(&yardlog::Handle::parse("intro").unwrap()).unwrap();
    assert_eq!(intro.versions.len(), 2);
    let design = fs::read_to_string(
        tree.top()
            .join(".yard/main")
            .join(stage_file(log.stages[0].stage.key.stage())),
    )
    .unwrap();
    assert!(
        design.contains("Hello.") && design.contains("Hello again."),
        "{design}"
    );
}

#[test]
fn writing_yard_log_that_has_not_changed_leaves_its_bytes_as_they_were() {
    let tree = Tree::new();
    let store = planned(&tree);
    let before = tree.scratch.snapshot();
    let updated = store
        .update(|log| -> Result<_, Failed> { Ok((log.unwrap(), ())) })
        .unwrap();
    assert_eq!(updated.written, Vec::<String>::new());
    assert_eq!(tree.scratch.snapshot(), before);
}

#[test]
fn a_change_to_one_block_rewrites_only_its_stage_s_file_and_the_root() {
    let tree = Tree::new();
    let store = planned(&tree);
    let build = stage_file(read(&store).stages[1].stage.key.stage());
    let updated = write(
        &store,
        &json!([{"id": "steps", "kind": "note", "stage": "build", "markdown": "More steps."}]),
    );
    assert_eq!(
        updated.written,
        [
            format!(".yard/main/{build}"),
            ".yard/main/yardlog.yaml".to_owned()
        ]
    );
}

#[test]
fn files_that_do_not_read_refuse_every_write_naming_the_file_and_line() {
    let tree = Tree::new();
    let store = planned(&tree);
    let name = stage_file(read(&store).stages[1].stage.key.stage());
    let path = tree.top().join(".yard/main").join(&name);
    let text = fs::read_to_string(&path).unwrap();
    let broken = text.replacen("blocks:", "<<<<<<< ours\nblocks:", 1);
    fs::write(&path, broken).unwrap();
    let before = tree.scratch.snapshot();
    let failed = try_write(
        &store,
        &json!([{"id": "intro", "kind": "note", "stage": "design", "markdown": "No."}]),
    )
    .unwrap_err();
    let Failed::Store(error @ YardStoreError::Unreadable { .. }) = failed else {
        panic!("{failed:?}");
    };
    let shown = error.to_string();
    assert!(shown.starts_with(&format!(".yard/main/{name}:")), "{shown}");
    assert!(
        matches!(&error, YardStoreError::Unreadable { error, .. } if error.error.position.is_some()),
        "{shown}"
    );
    assert_eq!(tree.scratch.snapshot(), before);
    assert!(matches!(
        store.read(),
        Err(YardStoreError::Unreadable { .. })
    ));
}

#[test]
fn a_root_that_does_not_read_refuses_every_write() {
    let tree = Tree::new();
    let store = planned(&tree);
    tree.scratch
        .put("tree/.yard/main/yardlog.yaml", "format: [yardlog/1\n");
    let before = tree.scratch.snapshot();
    let failed =
        try_write(&store, &json!([{"id": "x", "kind": "stage", "title": "X"}])).unwrap_err();
    assert!(
        matches!(&failed, Failed::Store(error @ YardStoreError::Unreadable { .. })
            if error.to_string().starts_with(".yard/main/yardlog.yaml:")),
        "{failed:?}"
    );
    assert_eq!(tree.scratch.snapshot(), before);
}

#[test]
fn yard_log_of_a_newer_format_is_read_as_newer_and_refuses_every_write() {
    let tree = Tree::new();
    let store = planned(&tree);
    let path = tree.top().join(".yard/main/yardlog.yaml");
    let text = fs::read_to_string(&path).unwrap();
    fs::write(&path, text.replacen("yardlog/1", "yardlog/9", 1)).unwrap();
    let before = tree.scratch.snapshot();
    assert_eq!(
        store.read().unwrap(),
        yardlog_store::Held::Newer(yardlog::Format::new(9))
    );
    let failed =
        try_write(&store, &json!([{"id": "x", "kind": "stage", "title": "X"}])).unwrap_err();
    let Failed::Store(error) = failed else {
        panic!("{failed:?}")
    };
    let shown = error.to_string();
    assert!(
        shown.contains("yardlog/9") && shown.contains("yardlog/1"),
        "{shown}"
    );
    assert_eq!(tree.scratch.snapshot(), before);
}

#[test]
fn a_write_whose_change_is_refused_changes_no_file() {
    let tree = Tree::new();
    let store = planned(&tree);
    let before = tree.scratch.snapshot();
    // A note written again under another stage than the one that holds it is refused.
    let failed = try_write(
        &store,
        &json!([{"id": "intro", "kind": "note", "stage": "build", "markdown": "Moved."}]),
    )
    .unwrap_err();
    let Failed::Refused(refusal) = failed else {
        panic!("{failed:?}")
    };
    assert!(
        matches!(refusal, yardlog::write::Refusal::Faults(_)),
        "{refusal}"
    );
    assert_eq!(tree.scratch.snapshot(), before);
}
