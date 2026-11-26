//! The outbox, `~/.yard/outbox/`: the write that seals a version puts a copy of it there, ready
//! to be sent as it is, for the address of one Yard Server, and the copies are taken for that
//! address alone, oldest first, with what cannot be sent moved aside or passed over.

use std::fs;

use serde_json::{Value, json};
use yardlog::batch::FORMAT as BATCH;
use yardlog_store::home::Home;
use yardlog_store::outbox::{FORMAT, Outbox};

use crate::common::{Failed, Scratch, Tree, read, try_write, write};

const ENDPOINT: &str = "http://127.0.0.1:7420";

fn outbox(scratch: &Scratch, endpoint: &str) -> Outbox {
    Home::at(scratch.0.join(".yard")).outbox(endpoint)
}

/// The store of `feat/x`'s Yard Log, copying new versions into the outbox for [`ENDPOINT`].
fn connected_store(tree: &Tree) -> yardlog_store::YardStore {
    tree.store("feat/x")
        .with_outbox(Some(outbox(&tree.scratch, ENDPOINT)))
}

/// The entries in the outbox's directory, by file name, read as JSON.
fn entries(scratch: &Scratch) -> Vec<(String, Value)> {
    let dir = scratch.0.join(".yard/outbox");
    let Ok(listing) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut found: Vec<(String, Value)> = listing
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_file())
        .map(|path| {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let text = fs::read_to_string(&path).unwrap();
            (name, serde_json::from_str(&text).unwrap())
        })
        .collect();
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found
}

/// Every version key Yard Log holds, oldest first.
fn version_keys(tree: &Tree, branch: &str) -> Vec<String> {
    let log = read(&tree.store(branch));
    let mut keys: Vec<String> = log
        .versions()
        .map(|(_, _, version)| version.key.to_string())
        .collect();
    keys.sort();
    keys
}

#[test]
fn a_write_puts_one_entry_for_each_version_it_adds_stages_included() {
    let tree = Tree::new();
    let store = connected_store(&tree);
    write(
        &store,
        &json!([
            {"id": "s1", "kind": "stage", "title": "S", "status": "active"},
            {"id": "n1", "kind": "note", "stage": "s1", "markdown": "Hello."}
        ]),
    );
    write(
        &store,
        &json!([{"id": "n1", "kind": "note", "stage": "s1", "markdown": "Again."}]),
    );
    let again = write(
        &store,
        &json!([{"id": "n1", "kind": "note", "stage": "s1", "markdown": "Once more."}]),
    );
    assert_eq!(again.outboxed, 1);
    let found = entries(&tree.scratch);
    let names: Vec<String> = found.iter().map(|(name, _)| name.clone()).collect();
    let expected: Vec<String> = version_keys(&tree, "feat/x")
        .iter()
        .map(|key| format!("{key}.json"))
        .collect();
    assert_eq!(names, expected);
    let kinds: Vec<&str> = found
        .iter()
        .map(|(_, entry)| entry["record"]["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["stage", "note", "note", "note"]);
}

#[test]
fn an_entry_holds_its_format_its_address_and_the_record_of_a_batch_and_nothing_else() {
    let tree = Tree::new();
    let store = connected_store(&tree);
    write(
        &store,
        &json!([{"id": "s1", "kind": "stage", "title": "S"}]),
    );
    let (name, entry) = entries(&tree.scratch).remove(0);
    assert_eq!(entry["format"], FORMAT);
    assert_eq!(FORMAT, "yardlog-outbox/1");
    assert_eq!(entry["to"], json!({"endpoint": ENDPOINT}));
    assert_eq!(entry["record"]["record"], "version");
    assert_eq!(
        format!(
            "{}.json",
            entry["record"]["version"]["key"].as_str().unwrap()
        ),
        name
    );
    let mut members: Vec<&String> = entry.as_object().unwrap().keys().collect();
    members.sort();
    assert_eq!(members, ["format", "record", "to"]);
    let text = fs::read_to_string(tree.scratch.0.join(".yard/outbox").join(&name)).unwrap();
    let top = tree.top().to_string_lossy().into_owned();
    for absent in [top.as_str(), "feat/x", "token"] {
        assert!(!text.contains(absent), "{absent}: {text}");
    }
}

#[test]
fn without_an_outbox_a_write_puts_no_entry() {
    let tree = Tree::new();
    write(
        &tree.store("feat/x"),
        &json!([{"id": "s1", "kind": "stage", "title": "S"}]),
    );
    assert_eq!(entries(&tree.scratch), Vec::new());
}

#[test]
fn a_refused_write_and_a_write_that_adds_no_version_put_no_entry() {
    let tree = Tree::new();
    let store = connected_store(&tree);
    write(
        &store,
        &json!([{"id": "s1", "kind": "stage", "title": "S"}]),
    );
    let before = entries(&tree.scratch);
    let refused = try_write(
        &store,
        &json!([{"id": "n1", "kind": "note", "stage": "missing", "markdown": "x"}]),
    );
    assert!(refused.is_err());
    let unchanged = store.update(|current| Ok::<_, Failed>((current.unwrap(), ())));
    assert!(unchanged.is_ok());
    assert_eq!(entries(&tree.scratch), before);
}

#[cfg(unix)]
#[test]
fn a_write_that_cannot_be_saved_puts_no_entry() {
    use std::os::unix::fs::PermissionsExt;

    let tree = Tree::new();
    let store = connected_store(&tree);
    write(
        &store,
        &json!([{"id": "s1", "kind": "stage", "title": "S"}]),
    );
    let before = entries(&tree.scratch);
    let folder = store.folder();
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o555)).unwrap();
    let unsaved = try_write(
        &store,
        &json!([{"id": "n1", "kind": "note", "stage": "s1", "markdown": "x"}]),
    );
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(unsaved.is_err());
    assert_eq!(entries(&tree.scratch), before);
}

#[test]
fn an_outbox_that_cannot_be_written_keeps_the_write_and_names_the_version_left_out() {
    let tree = Tree::new();
    tree.scratch
        .put(".yard/outbox", "a file where the directory should be");
    let store = connected_store(&tree);
    let updated = write(
        &store,
        &json!([{"id": "s1", "kind": "stage", "title": "S"}]),
    );
    let key = version_keys(&tree, "feat/x").remove(0);
    assert_eq!(updated.outboxed, 0);
    let unsent = updated.unsent.expect("a warning");
    assert!(unsent.contains("outbox"), "{unsent}");
    assert!(unsent.contains(&key), "{unsent}");
}

#[test]
fn entries_are_taken_oldest_first_for_their_own_address_alone() {
    let tree = Tree::new();
    let ours = outbox(&tree.scratch, ENDPOINT);
    let theirs = outbox(&tree.scratch, "https://elsewhere.example");
    let store = tree.store("feat/x").with_outbox(Some(theirs));
    write(
        &store,
        &json!([{"id": "s0", "kind": "stage", "title": "S"}]),
    );
    let store = tree.store("feat/x").with_outbox(Some(ours.clone()));
    for n in 1..=3 {
        write(
            &store,
            &json!([{"id": format!("s{n}"), "kind": "stage", "title": "S"}]),
        );
    }
    let all = entries(&tree.scratch);
    let taken = ours.take(2).unwrap();
    let keys: Vec<&str> = taken
        .entries
        .iter()
        .map(|entry| entry.key.as_str())
        .collect();
    let ours_keys: Vec<String> = all[1..]
        .iter()
        .map(|(name, _)| name.trim_end_matches(".json").to_owned())
        .collect();
    assert_eq!(keys, [ours_keys[0].as_str(), ours_keys[1].as_str()]);
    assert_eq!(taken.waiting, 3);
    let record: Value = serde_json::from_str(&taken.entries[0].record).unwrap();
    assert_eq!(record, all[1].1["record"]);
    assert_eq!(entries(&tree.scratch), all);
}

#[test]
fn a_record_is_taken_with_its_fields_in_the_order_written() {
    let tree = Tree::new();
    let ours = outbox(&tree.scratch, ENDPOINT);
    let store = tree.store("feat/x").with_outbox(Some(ours.clone()));
    write(
        &store,
        &json!([{"id": "s1", "kind": "stage", "title": "S"}]),
    );
    let taken = ours.take(16).unwrap();
    assert!(
        taken.entries[0]
            .record
            .starts_with("{\"record\":\"version\",\"yardLog\":"),
        "{}",
        taken.entries[0].record
    );
    let batch: Value =
        serde_json::from_str(&yardlog::batch::batch(&[&taken.entries[0].record])).unwrap();
    assert_eq!(batch["format"], BATCH);
}

#[test]
fn removed_entries_leave_the_outbox_and_one_already_gone_is_no_error() {
    let tree = Tree::new();
    let ours = outbox(&tree.scratch, ENDPOINT);
    let store = tree.store("feat/x").with_outbox(Some(ours.clone()));
    write(
        &store,
        &json!([{"id": "s1", "kind": "stage", "title": "S"}]),
    );
    write(
        &store,
        &json!([{"id": "s2", "kind": "stage", "title": "S"}]),
    );
    let taken = ours.take(16).unwrap();
    ours.remove(&taken.entries[..1]).unwrap();
    ours.remove(&taken.entries[..1]).unwrap();
    assert_eq!(entries(&tree.scratch).len(), 1);
}

#[test]
fn a_broken_file_and_a_foreign_json_file_are_moved_to_dead_and_the_rest_are_taken() {
    let scratch = Scratch::new();
    let ours = outbox(&scratch, ENDPOINT);
    scratch.put(
        ".yard/outbox/00000000-0000-7000-8000-000000000001.json",
        "{not json",
    );
    scratch.put(
        ".yard/outbox/00000000-0000-7000-8000-000000000002.json",
        r#"{"format": "other/1"}"#,
    );
    scratch.put(
        ".yard/outbox/00000000-0000-7000-8000-000000000003.json",
        &entry(ENDPOINT, "3", 1),
    );
    scratch.put(
        ".yard/outbox/.00000000-0000-7000-8000-000000000004.json.1.0.tmp",
        "{half",
    );
    let taken = ours.take(16).unwrap();
    assert_eq!(taken.dead, 2);
    assert_eq!(taken.entries.len(), 1);
    assert_eq!(taken.entries[0].key, "00000000-0000-7000-8000-000000000003");
    let dead = scratch.0.join(".yard/outbox/dead");
    assert!(
        dead.join("00000000-0000-7000-8000-000000000001.json")
            .is_file()
    );
    assert!(
        dead.join("00000000-0000-7000-8000-000000000002.json")
            .is_file()
    );
    let again = ours.take(16).unwrap();
    assert_eq!((again.dead, again.entries.len()), (0, 1));
}

#[test]
fn an_entry_of_a_newer_format_stays_and_is_passed_over() {
    let scratch = Scratch::new();
    let ours = outbox(&scratch, ENDPOINT);
    scratch.put(
        ".yard/outbox/00000000-0000-7000-8000-000000000001.json",
        &entry(ENDPOINT, "1", 2),
    );
    scratch.put(
        ".yard/outbox/00000000-0000-7000-8000-000000000002.json",
        &entry(ENDPOINT, "2", 1),
    );
    let taken = ours.take(16).unwrap();
    assert_eq!(taken.skipped, 1);
    assert_eq!(taken.entries.len(), 1);
    assert_eq!(taken.entries[0].key, "00000000-0000-7000-8000-000000000002");
    assert!(
        scratch
            .0
            .join(".yard/outbox/00000000-0000-7000-8000-000000000001.json")
            .is_file()
    );
}

#[test]
fn an_empty_or_missing_outbox_has_nothing_to_take() {
    let scratch = Scratch::new();
    let taken = outbox(&scratch, ENDPOINT).take(16).unwrap();
    assert_eq!(
        (
            taken.entries.len(),
            taken.waiting,
            taken.skipped,
            taken.dead
        ),
        (0, 0, 0, 0)
    );
}

/// An entry of `yardlog-outbox/<version>` for `endpoint`, whose record names `id`.
fn entry(endpoint: &str, id: &str, version: u32) -> String {
    json!({
        "format": format!("yardlog-outbox/{version}"),
        "to": {"endpoint": endpoint},
        "record": {"record": "version", "id": id, "version": {"key": id}},
    })
    .to_string()
}

#[test]
fn a_later_write_puts_no_entry_again_for_a_version_already_sent() {
    let tree = Tree::new();
    let ours = outbox(&tree.scratch, ENDPOINT);
    let store = tree.store("feat/x").with_outbox(Some(ours.clone()));
    write(
        &store,
        &json!([{"id": "s1", "kind": "stage", "title": "S"}]),
    );
    let sent = ours.take(16).unwrap();
    ours.remove(&sent.entries).unwrap();
    write(
        &store,
        &json!([{"id": "s2", "kind": "stage", "title": "T"}]),
    );
    let left = entries(&tree.scratch);
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].1["record"]["id"], "s2");
}

#[test]
fn the_person_s_answer_puts_no_entry() {
    let tree = Tree::new();
    let store = connected_store(&tree);
    write(
        &store,
        &json!([
            {"id": "s1", "kind": "stage", "title": "S"},
            {"id": "q1", "kind": "question", "stage": "s1", "prompt": "Which?", "header": "Pick",
             "options": [{"label": "A"}, {"label": "B"}]}
        ]),
    );
    let before = entries(&tree.scratch);
    let answered = store.update(|current| {
        let mut log = current.unwrap();
        let block = log
            .find(&yardlog::Handle::parse("q1").unwrap())
            .unwrap()
            .key;
        let reply = yardlog::respond::Reply {
            block,
            given: yardlog::log::Given::Answer {
                chosen: vec!["A".to_owned()],
                words: None,
                notes: None,
            },
        };
        let who = yardlog::respond::Answering {
            at: crate::common::at(9),
            name: "Ada".to_owned(),
            email: "ada@example.com".to_owned(),
            channel: yardlog::Channel::Terminal,
        };
        log.answer(&[reply], &who).unwrap();
        Ok::<_, Failed>((log, ()))
    });
    assert!(answered.is_ok());
    assert_eq!(entries(&tree.scratch), before);
}

#[cfg(unix)]
#[test]
fn a_file_that_cannot_be_read_is_passed_over_and_the_rest_are_taken() {
    use std::os::unix::fs::PermissionsExt as _;
    let scratch = Scratch::new();
    let ours = outbox(&scratch, ENDPOINT);
    let locked = ".yard/outbox/00000000-0000-7000-8000-000000000001.json";
    scratch.put(locked, &entry(ENDPOINT, "1", 1));
    scratch.put(
        ".yard/outbox/00000000-0000-7000-8000-000000000002.json",
        &entry(ENDPOINT, "2", 1),
    );
    let path = scratch.0.join(locked);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
    let readable = fs::read(&path).is_ok();
    let taken = ours.take(16);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    let taken = taken.unwrap();
    if !readable {
        assert_eq!(taken.entries.len(), 1);
        assert_eq!(taken.entries[0].key, "00000000-0000-7000-8000-000000000002");
    }
}
