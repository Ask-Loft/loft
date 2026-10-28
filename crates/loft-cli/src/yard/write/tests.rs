//! Locked application with time advanced by the clock simulator, never a real wait.

use std::time::Duration;

use loft_ports::clock::{Clock as _, SystemClock};
use serde_json::json;
use yardlog::input::read_batch;
use yardlog::write::{Context, Reports};
use yardlog::{Agent, Time};

use super::apply;

#[test]
fn a_prepared_batch_reads_its_write_time_only_when_it_is_applied() {
    let clock = SystemClock::with_epoch(Some("1000")).passing_if(true);
    let at = || Time::from_unix_seconds(clock.now().unwrap()).unwrap();
    let context = Context {
        at: at(),
        agent: Agent::main("caller"),
        base: None,
        send: true,
    };
    let inputs = read_batch(&json!([
        {"id":"stage","kind":"stage","status":"active","title":"Stage"},
        {"id":"ask","kind":"approval","stage":"stage","title":"Proceed?","markdown":"Ready"}
    ]))
    .unwrap();
    // Report preparation and lock acquisition may take time after the request began.
    clock.wait(Duration::from_secs(9));
    let (log, _) = apply(
        None,
        ("feat/shelf", "develop"),
        &inputs,
        context,
        &Reports::new(),
        || Ok(at()),
    )
    .unwrap_or_else(|_| panic!("the valid batch was refused"));
    assert_eq!(log.root.opened.at.to_string(), "1970-01-01T00:16:49Z");
    assert!(log.blocks().all(|block| block.current().act.at == at()));
    assert_eq!(log.root.submissions[0].sending().at, at());
}
