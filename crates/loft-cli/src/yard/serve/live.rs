//! What the page server follows while it runs, four times a second through the clock port: a
//! newer `loft`'s request that it stop, which it heeds by finishing the responses it is sending
//! and exiting; and the files of every Yard Log it serves, whoever changes them, a `loft yard`
//! command or git, each change of which it counts so that open pages hear of it. It looks at the
//! trees and their current Yard Logs every half second, and sweeps every other Yard Log every 5
//! seconds (`watch`).

use std::collections::BTreeMap;
use std::sync::atomic::Ordering;
use std::time::Duration;

use loft_ports::clock::Clock as _;

use super::Server;
use super::discovery;
use super::watch::Watch;
use crate::yard::clock;

/// How often the page server looks.
pub const TICK: Duration = Duration::from_millis(250);

/// How many ticks pass between two looks at the trees and their current Yard Logs.
const QUICK: u64 = 2;

/// How many ticks pass between two sweeps of every other Yard Log.
const SWEEP: u64 = 20;

/// The longest the page server waits for the responses it is sending before it exits.
const FINISHING: Duration = Duration::from_millis(600);

/// Follows the stop requests and the files until the page server exits.
pub fn follow(server: &Server) -> ! {
    let clock = clock();
    let mut last: Option<Digests> = None;
    let mut watch = Watch::default();
    let mut tick: u64 = 0;
    loop {
        clock.wait(TICK);
        if discovery::asked_to_stop(&server.home) {
            stop(server);
        }
        tick += 1;
        if tick.is_multiple_of(QUICK) {
            let sweep = last.is_none() || tick.is_multiple_of(SWEEP);
            let now = watch.look(&server.home, sweep);
            let changed = last.as_ref().map(|last| changed(last, &now));
            if let Some(changed) = changed.filter(|changed| !changed.is_empty()) {
                server.changed(changed);
            }
            last = Some(now);
        }
    }
}

/// What the page shows, by what of it changes together: the trees served and what each is on,
/// and the files of each branch's Yard Log, apart from those its sessions keep.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Area {
    Trees,
    Log { tree: String, branch: String },
    Sessions { tree: String, branch: String },
}

pub type Digests = BTreeMap<Area, u64>;

/// The areas whose digest differs between `last` and `now`, or that one of them lacks.
fn changed(last: &Digests, now: &Digests) -> Vec<Area> {
    let mut areas: Vec<Area> = now
        .iter()
        .filter(|(area, digest)| last.get(area) != Some(digest))
        .map(|(area, _)| area.clone())
        .collect();
    areas.extend(last.keys().filter(|area| !now.contains_key(area)).cloned());
    areas
}

/// Finishes the responses being sent, then exits; the operating system frees the lock.
fn stop(server: &Server) -> ! {
    server.stopping.store(true, Ordering::SeqCst);
    let clock = clock();
    let began = clock.elapsed();
    while server.busy.load(Ordering::SeqCst) > 0
        && clock.elapsed().saturating_sub(began) < FINISHING
    {
        clock.wait(Duration::from_millis(10));
    }
    std::process::exit(0)
}
