//! One run of `loft yard flush`: it sends batch after batch until no entry for its address is
//! left that it has not taken, and only one run sends at a time for each home of Yard, under the
//! lock `~/.yard/flush.lock`, which no write takes.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};
use software_plant::YardServer;
use yardlog_store::lock;

use crate::yard::{Yard, document};
use crate::yard_connected::{STAGE_BATCH, connected, entries, flush, held, notes, outbox, write};

/// The lock of the flush on the machine `yard` runs on.
fn flush_lock(yard: &Yard) -> PathBuf {
    yard.scratch.home().join(".yard/flush.lock")
}

/// `loft yard flush --format json` under `variables`, to run on another thread.
fn flushing(yard: &Yard, variables: &[(&'static str, String)]) -> Command {
    let mut command = yard.scratch.command(Some(&yard.git), &yard.clock);
    command
        .envs(variables.iter().cloned())
        .args(["yard", "flush", "--format", "json"]);
    command
}

/// The document of a flush run on another thread.
fn flushed(mut command: Command) -> Value {
    document("flush", &command.output().unwrap())
}

/// The ids of the records each request carried, request by request.
fn batches(server: &YardServer) -> Vec<Vec<String>> {
    server
        .requests()
        .iter()
        .map(|request| {
            let batch: Value = serde_json::from_slice(&request.body).unwrap();
            batch["records"]
                .as_array()
                .unwrap()
                .iter()
                .map(|record| record["id"].as_str().unwrap().to_owned())
                .collect()
        })
        .collect()
}

#[test]
fn one_flush_sends_40_entries_in_three_batches_and_ends_once_none_is_left() {
    let yard = Yard::new();
    let server = YardServer::accepting();
    let variables = connected(&server.url());
    write(&yard, STAGE_BATCH, &[]);
    write(&yard, &notes(40), &variables);
    let (code, flushed) = flush(&yard, &variables);
    assert_eq!((code, &flushed["outcome"]), (Some(0), &json!("sent")));
    let sizes: Vec<usize> = batches(&server).iter().map(Vec::len).collect();
    assert_eq!(sizes, [16, 16, 8]);
    assert_eq!(
        (&flushed["sent"], &flushed["remaining"]),
        (&json!(40), &json!(0))
    );
    assert_eq!(entries(&yard), Vec::<Value>::new());
}

#[test]
fn an_entry_put_while_a_batch_is_on_its_way_goes_in_a_later_batch_of_the_same_run() {
    let yard = Yard::new();
    let server = YardServer::late(Duration::from_millis(1500));
    let variables = connected(&server.url());
    write(&yard, STAGE_BATCH, &variables);
    let command = flushing(&yard, &variables);
    let running = thread::spawn(move || flushed(command));
    while server.requests().is_empty() && !running.is_finished() {
        thread::sleep(Duration::from_millis(20));
    }
    write(&yard, &notes(1), &variables);
    let result = running.join().unwrap();
    assert_eq!(result["outcome"], "sent", "{result}");
    assert_eq!(batches(&server), [vec!["build"], vec!["n1"]]);
    assert_eq!(entries(&yard), Vec::<Value>::new());
}

#[test]
fn the_body_of_a_batch_is_no_larger_than_the_entry_files_it_carries() {
    let yard = Yard::new();
    let server = YardServer::accepting();
    let variables = connected(&server.url());
    write(&yard, STAGE_BATCH, &variables);
    write(&yard, &notes(3), &variables);
    let files: u64 = fs::read_dir(outbox(&yard))
        .unwrap()
        .map(|entry| entry.unwrap().metadata().unwrap().len())
        .sum();
    flush(&yard, &variables);
    let body = server.requests()[0].body.len() as u64;
    assert!(body <= files, "{body} over {files}");
}

#[test]
fn a_flush_that_finds_the_lock_held_is_busy_makes_no_request_and_exits_0() {
    let yard = Yard::new();
    let server = YardServer::accepting();
    let variables = connected(&server.url());
    write(&yard, STAGE_BATCH, &variables);
    let held_lock = lock::try_lock(&flush_lock(&yard)).unwrap().unwrap();
    let (code, flushed) = flush(&yard, &variables);
    assert_eq!((code, &flushed["outcome"]), (Some(0), &json!("busy")));
    assert_eq!(server.requests().len(), 0);
    assert_eq!(entries(&yard).len(), 1);
    drop(held_lock);
    let (_, flushed) = flush(&yard, &variables);
    assert_eq!(flushed["outcome"], "sent");
}

#[test]
fn the_flush_that_holds_the_lock_sends_every_batch_while_another_is_busy() {
    let yard = Yard::new();
    let server = YardServer::late(Duration::from_millis(800));
    let variables = connected(&server.url());
    write(&yard, STAGE_BATCH, &[]);
    write(&yard, &notes(17), &variables);
    let command = flushing(&yard, &variables);
    let running = thread::spawn(move || flushed(command));
    while server.requests().is_empty() {
        thread::sleep(Duration::from_millis(20));
    }
    assert!(lock::is_held(&flush_lock(&yard)).unwrap());
    let (code, second) = flush(&yard, &variables);
    let first = running.join().unwrap();
    assert_eq!((code, &second["outcome"]), (Some(0), &json!("busy")));
    assert_eq!(first["outcome"], "sent");
    assert_eq!(first["sent"], 17);
    assert_eq!(held(&server).len(), 17);
    assert_eq!(server.requests().len(), 2);
}

#[test]
fn once_the_flush_that_held_the_lock_is_stopped_the_next_flush_runs() {
    let yard = Yard::new();
    let silent = YardServer::silent();
    let variables = connected(&silent.url());
    write(&yard, STAGE_BATCH, &variables);
    let mut running = flushing(&yard, &variables).spawn().unwrap();
    while silent.requests().is_empty() {
        thread::sleep(Duration::from_millis(20));
    }
    assert!(lock::is_held(&flush_lock(&yard)).unwrap());
    running.kill().unwrap();
    running.wait().unwrap();
    let accepting = YardServer::accepting();
    let (code, flushed) = flush(&yard, &connected(&accepting.url()));
    assert_eq!((code, &flushed["outcome"]), (Some(0), &json!("empty")));
}

#[test]
fn a_write_is_not_held_up_by_a_flush_that_holds_the_lock() {
    let yard = Yard::new();
    let server = YardServer::accepting();
    let variables = connected(&server.url());
    let _held = lock::try_lock(&flush_lock(&yard)).unwrap().unwrap();
    let output = write(&yard, STAGE_BATCH, &variables);
    assert_eq!(document("write", &output)["outcome"], "written");
    assert_eq!(entries(&yard).len(), 1);
}

#[test]
fn two_homes_of_yard_have_a_lock_each() {
    let yard = Yard::new();
    let server = YardServer::accepting();
    let other = yard.scratch.home().join("other-yard");
    let _held = lock::try_lock(&flush_lock(&yard)).unwrap().unwrap();
    let mut variables = connected(&server.url()).to_vec();
    variables.push(("LOFT_YARD_HOME", other.to_string_lossy().into_owned()));
    write(&yard, STAGE_BATCH, &variables);
    let (code, flushed) = flush(&yard, &variables);
    assert_eq!((code, &flushed["outcome"]), (Some(0), &json!("sent")));
    assert!(other.join("flush.lock").is_file());
}
