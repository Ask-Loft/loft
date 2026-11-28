//! A version an agent writes reaches Yard Server end to end: the real `loft` binary writes Yard
//! Log in connected mode and flushes the outbox, and the version Yard Server holds is the one
//! Yard Log holds, field for field. Where Yard Server stores a batch and its answer is lost, the
//! next flush sends it again and Yard Server still holds one copy.

use serde_json::Value;
use software_plant::YardServer;

use crate::yard::{Yard, block, document};
use crate::yard_connected::{STAGE_BATCH, come_back_after, connected, entries, flush, held, write};

#[test]
fn a_note_written_by_loft_arrives_in_yard_server_as_yard_log_holds_it() {
    let yard = Yard::new();
    let server = YardServer::accepting();
    let variables = connected(&server.url());
    write(&yard, STAGE_BATCH, &variables);
    let output = write(
        &yard,
        r#"[{"id":"n1","kind":"note","stage":"build","title":"Hello","markdown":"Two\nlines.","folded":true}]"#,
        &variables,
    );
    assert_eq!(document("write", &output)["outcome"], "written");
    flush(&yard, &variables);
    assert_eq!(held(&server), ["build", "n1"]);
    let read = yard.read("s1", &[]);
    let note = block(&read, "n1");
    let arrived = &server.records()[1];
    assert_eq!(arrived["record"], "version");
    assert_eq!(arrived["kind"], "note");
    assert_eq!(arrived["id"], "n1");
    assert_eq!(arrived["block"], note["key"]);
    assert_eq!(arrived["yardLog"], read["yardLog"]["key"]);
    let version = &arrived["version"];
    assert_eq!(version["key"], note["version"]["key"]);
    assert_eq!(version["at"], note["version"]["act"]["at"]);
    assert_eq!(version["revision"], note["version"]["act"]["revision"]);
    assert_eq!(version["by"], note["version"]["act"]["by"]);
    for field in ["title", "markdown", "folded"] {
        assert_eq!(version[field], note["version"]["body"][field], "{field}");
    }
    assert_eq!(entries(&yard), Vec::<Value>::new());
}

#[test]
fn a_batch_stored_and_unanswered_is_sent_again_by_the_next_flush_and_held_once() {
    let yard = Yard::new();
    let server = YardServer::hanging_up();
    let variables = connected(&server.url());
    write(&yard, STAGE_BATCH, &variables);
    flush(&yard, &variables);
    assert_eq!(held(&server), ["build"]);
    assert_eq!(entries(&yard).len(), 1);
    come_back_after(
        &yard,
        &server.url(),
        crate::common::signed_in::real_now() - 1,
    );
    let (code, flushed) = flush(&yard, &variables);
    assert_eq!(
        (code, &flushed["outcome"]),
        (Some(1), &Value::from("failed"))
    );
    assert_eq!(server.requests().len(), 2);
    assert_eq!(held(&server), ["build"]);
    assert_eq!(server.records().len(), 1);
}
