//! What connected mode never breaks: a write whose outbox cannot be written gives the result it
//! gives without connected mode, the token goes to Yard Server alone, and an `https` address is
//! contacted where an `http` one off the loopback interface is not.

use std::fs;

use serde_json::Value;
use software_plant::releases::Release;
use software_plant::yard_server::TOKEN;
use software_plant::{Collector, ReleaseFeed, YardServer};

use crate::common::stderr;
use crate::yard::{Yard, document};
use crate::yard_connected::{STAGE_BATCH, connected, flush, held, notes, outbox, run, write};

/// A write's document, with what differs between two trees left out: the paths and the keys.
fn comparable(mut written: Value) -> Value {
    for field in ["tree", "commonDir"] {
        written[field] = Value::Null;
    }
    for block in written["blocks"].as_array_mut().unwrap() {
        block["key"] = Value::Null;
    }
    written
}

#[test]
fn a_write_whose_entries_cannot_be_put_gives_the_result_it_gives_without_connected_mode() {
    let plain = Yard::new();
    let offline = write(&plain, STAGE_BATCH, &[]);
    let yard = Yard::new();
    fs::create_dir_all(yard.scratch.home().join(".yard")).unwrap();
    fs::write(outbox(&yard), "a file where the directory should be").unwrap();
    let server = YardServer::accepting();
    let online = write(&yard, STAGE_BATCH, &connected(&server.url()));
    assert_eq!(online.status.code(), offline.status.code());
    assert_eq!(
        comparable(document("write", &online)),
        comparable(document("write", &offline))
    );
    let said = stderr(&online);
    assert!(said.contains("could not take version"), "{said}");
}

#[test]
fn the_token_goes_to_yard_server_and_to_no_other_address_a_command_reaches() {
    let yard = Yard::new();
    let server = YardServer::accepting();
    let collector = Collector::accepting();
    let feed = ReleaseFeed::listing(vec![Release::published("v99.0.0")]);
    let mut variables = connected(&server.url()).to_vec();
    variables.extend([
        ("LOFT_TELEMETRY", "internal".to_owned()),
        ("LOFT_TELEMETRY_ENDPOINT", collector.url()),
        ("LOFT_UPDATE_CHECK", String::new()),
        ("LOFT_UPDATE_ENDPOINT", feed.url()),
    ]);
    for batch in [STAGE_BATCH, &notes(1)] {
        write(&yard, batch, &variables);
    }
    run(&yard, &["flush"], &variables);
    assert_eq!(held(&server), ["build", "n1"]);
    assert!(
        server
            .requests()
            .iter()
            .all(|request| request.header("Authorization") == Some(&format!("Bearer {TOKEN}")))
    );
    assert_eq!(collector.requests().len(), 0);
    assert_eq!(feed.requests().len(), 0);
}

#[test]
fn an_https_address_is_contacted_where_one_off_the_loopback_interface_over_http_is_not() {
    let yard = Yard::new();
    let https = YardServer::refusing().replacen("http://", "https://", 1);
    write(&yard, STAGE_BATCH, &connected(&https));
    let (code, flushed) = flush(&yard, &connected(&https));
    assert_eq!(
        (code, flushed["outcome"].as_str()),
        (Some(1), Some("failed"))
    );
    let why = flushed["errors"].to_string();
    assert!(!why.contains("on the loopback interface"), "{why}");
    let plain = Yard::new();
    let http = "http://yard.example";
    write(&plain, STAGE_BATCH, &connected(http));
    let (_, refused) = flush(&plain, &connected(http));
    let why = refused["errors"].to_string();
    assert!(why.contains("on the loopback interface"), "{why}");
}
