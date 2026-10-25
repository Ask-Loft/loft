//! The Yard commands, and the crates they read and write Yard Log through, tell the time and wait
//! only through the clock port of `loft-ports`, so that the Software Plant's clock simulator
//! stands in for every reading and every wait. No source of theirs names the system's clock or
//! a sleep. Elsewhere in the command line, the network client and the telemetry bound and measure
//! real network calls by the real clock, which no simulated time may shorten; they are left out.

use std::fs;
use std::path::Path;

fn sources(dir: &Path, found: &mut Vec<(String, String)>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sources(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            let text = fs::read_to_string(&path).unwrap();
            found.push((path.display().to_string(), text));
        }
    }
}

#[test]
fn the_yard_commands_tell_the_time_and_wait_only_through_the_clock_port() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut found = Vec::new();
    sources(&crates.join("loft-cli/src/yard"), &mut found);
    for library in ["yardlog", "yardlog-store", "loft-common", "loft-ports"] {
        sources(&crates.join(library).join("src"), &mut found);
    }
    assert!(found.len() > 40, "the scan found {} files", found.len());
    let port = Path::new("loft-ports").join("src").join("clock.rs");
    let mut named = Vec::new();
    for (path, text) in &found {
        if Path::new(path).ends_with(&port) {
            continue;
        }
        for item in ["Instant", "SystemTime", "thread::sleep"] {
            if text.contains(item) {
                named.push(format!("{path} names {item}"));
            }
        }
    }
    assert_eq!(named, Vec::<String>::new());
}
