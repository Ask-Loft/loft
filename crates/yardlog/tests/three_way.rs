//! Two copies of Yard Log merged as git merges a folder: a file both copies changed goes to the
//! driver, a file one copy changed is taken whole from it, and a file both changed alike is kept
//! as it is. What the driver never meets must still reach the settling.

use std::collections::BTreeMap;

use serde_json::json;
use yardlog::YardLog;
use yardlog::codec::ROOT;
use yardlog::log::Arrival;
use yardlog::merge::merge_file;

use crate::common::{context, handle, main_agent, other_session, planned, write};

fn files(log: &YardLog) -> BTreeMap<String, String> {
    log.files().into_iter().collect()
}

/// `ours` and `theirs` merged over `base` as git merges them with the driver registered, and
/// the names of the files git handed to the driver.
fn as_git_merges(base: &YardLog, ours: &YardLog, theirs: &YardLog) -> (YardLog, Vec<String>) {
    let (base, ours, theirs) = (files(base), files(ours), files(theirs));
    let mut result = BTreeMap::new();
    let mut driven = Vec::new();
    for name in ours.keys().chain(theirs.keys()) {
        let (o, t, b) = (ours.get(name), theirs.get(name), base.get(name));
        let text = match (o, t) {
            (Some(o), Some(t)) if o == t => o.clone(),
            (Some(o), Some(t)) if b == Some(o) => t.clone(),
            (Some(o), Some(t)) if b == Some(t) => o.clone(),
            (Some(o), Some(t)) => {
                driven.push(name.clone());
                merge_file(name, b.map(String::as_bytes), o.as_bytes(), t.as_bytes())
                    .unwrap_or_else(|(which, error)| panic!("{which}: {error}"))
            }
            (Some(one), None) | (None, Some(one)) => one.clone(),
            (None, None) => unreachable!(),
        };
        result.insert(name.clone(), text);
    }
    driven.dedup();
    let log = YardLog::read(result[ROOT].as_bytes(), |name| {
        result.get(name).map(|t| t.clone().into_bytes())
    })
    .unwrap();
    (log, driven)
}

fn note(id: &str, stage: &str) -> serde_json::Value {
    json!([{"id": id, "kind": "note", "stage": stage, "markdown": id}])
}

/// The base, planned with a second stage `build`; this copy writes a note to `design`, the other
/// one to `build`, so that each stage's file is changed on one side alone.
fn apart() -> (YardLog, YardLog, YardLog) {
    let mut base = planned();
    let build = json!([{"id": "build", "kind": "stage", "title": "Build"}]);
    write(&mut base, &build, &context(&main_agent(), 5));
    let (mut ours, mut theirs) = (base.clone(), base.clone());
    write(
        &mut ours,
        &note("mine", "design"),
        &context(&main_agent(), 10),
    );
    write(
        &mut theirs,
        &note("yours", "build"),
        &context(&other_session(), 20),
    );
    (base, ours, theirs)
}

#[test]
fn two_copies_that_each_changed_yard_log_hand_the_root_to_the_driver() {
    let (base, ours, theirs) = apart();
    let (merged, driven) = as_git_merges(&base, &ours, &theirs);
    assert_eq!(driven, [ROOT]);
    assert_eq!(merged.root.unsettled_since, Some(ours.root.revision));
}

#[test]
fn the_merged_root_lists_as_arriving_each_act_the_other_copy_holds_and_this_one_did_not() {
    let (base, ours, theirs) = apart();
    let (merged, _) = as_git_merges(&base, &ours, &theirs);
    let theirs_act = theirs.root.acts.last().unwrap();
    assert_eq!(merged.root.arriving, std::slice::from_ref(theirs_act));
    assert!(merged.root.acts.contains(theirs_act));
    assert!(merged.root.acts.contains(ours.root.acts.last().unwrap()));
}

#[test]
fn a_record_of_a_stage_s_file_git_took_whole_is_marked_as_arrived_at_the_settling() {
    let (base, ours, theirs) = apart();
    let (mut merged, _) = as_git_merges(&base, &ours, &theirs);
    let settled = merged
        .settle(crate::common::at(30), yardlog::Actor::Agent(main_agent()))
        .unwrap()
        .act
        .revision;
    let arrived = |id: &str| merged.find(&handle(id)).unwrap().ordered()[0].arrived;
    assert_eq!(arrived("yours"), Some(Arrival::At(settled)));
    assert_eq!(arrived("mine"), None);
    assert_eq!(merged.root.arriving, []);
}
