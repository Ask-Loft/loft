//! Local send arrival is distinct from the original Act revision after merging or take-up.

use serde_json::json;
use yardlog::input::read_batch;
use yardlog::log::{Arrival, Change, Happened};
use yardlog::write::{Context, Reports};
use yardlog::{Act, Actor, Agent, Time, YardLog};

use super::{super::caller::Continuations, submissions};

#[test]
fn an_open_submission_exposes_the_local_arrival_of_its_current_send() {
    let at = Time::from_unix_seconds(1000).unwrap();
    let agent = Agent::main("caller");
    let mut log = YardLog::open("feat/shelf", "develop", at, agent.clone());
    let inputs = read_batch(&json!([
        {"id":"stage","kind":"stage","status":"active","title":"Stage"},
        {"id":"ask","kind":"approval","stage":"stage","title":"Proceed?","markdown":"Ready"}
    ]))
    .unwrap();
    log.write(
        &inputs,
        &Context {
            at,
            agent: agent.clone(),
            base: None,
            send: true,
        },
        &Reports::new(),
    )
    .unwrap();
    log.root.submissions[0].changes[0].arrived = Some(Arrival::At(7));
    let shown = submissions(
        &log,
        &Continuations::read(None).unwrap_or_else(|_| panic!("empty continuations")),
    );
    assert_eq!(shown[0]["open"][0]["sentReached"], 7);
    assert_eq!(shown[0]["open"][0]["sent"]["revision"], 1);

    log.root.submissions[0].changes.push(Change {
        what: Happened::TakenUp,
        act: Act {
            at: Time::from_unix_seconds(1001).unwrap(),
            revision: 8,
            actor: Actor::Agent(agent),
        },
        arrived: Some(Arrival::At(9)),
    });
    let shown = submissions(
        &log,
        &Continuations::read(None).unwrap_or_else(|_| panic!("empty continuations")),
    );
    assert_eq!(shown[0]["open"][0]["sentReached"], 9);
    assert_eq!(shown[0]["open"][0]["sent"]["revision"], 8);
}
