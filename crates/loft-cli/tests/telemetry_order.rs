//! The order of a run's work: the command's whole output first, then the telemetry notice where
//! one is due, and only then a request to the collector or to GitHub. Each test holds the request
//! open at a silent service and reads what the run had written by the time the request arrived.

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::common::telemetry::{asking, seen, sending};
use crate::common::{EPOCH, Scratch};
use software_plant::{Clock, Collector, ReleaseFeed};

/// What a run had written when a service first counted its request, and all it wrote.
struct Observed {
    stdout_then: String,
    stderr_then: String,
    stdout: String,
}

/// Collects everything `pipe` yields into a buffer shared with the test.
fn collect(mut pipe: impl Read + Send + 'static) -> (Arc<Mutex<Vec<u8>>>, JoinHandle<()>) {
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let filled = Arc::clone(&buffer);
    let reader = thread::spawn(move || {
        let mut chunk = [0; 4096];
        while let Ok(read) = pipe.read(&mut chunk) {
            if read == 0 {
                break;
            }
            filled.lock().unwrap().extend_from_slice(&chunk[..read]);
        }
    });
    (buffer, reader)
}

fn text(buffer: &Mutex<Vec<u8>>) -> String {
    String::from_utf8(buffer.lock().unwrap().clone()).unwrap()
}

/// Runs `command`, whose services never answer, and gives what it had written when `asked`
/// first counted a request. A silent service holds the run until the network client's time
/// limit, so the readers are given a moment to take what the run wrote before it asked; what it
/// writes in that moment would be counted as written before.
fn observe(mut command: Command, asked: impl Fn() -> usize) -> Observed {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    let (stdout, out_reader) = collect(child.stdout.take().unwrap());
    let (stderr, err_reader) = collect(child.stderr.take().unwrap());
    let started = Instant::now();
    while asked() == 0 {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "no request came"
        );
        thread::sleep(Duration::from_millis(5));
    }
    thread::sleep(Duration::from_millis(100));
    let (stdout_then, stderr_then) = (text(&stdout), text(&stderr));
    child.wait().unwrap();
    out_reader.join().unwrap();
    err_reader.join().unwrap();
    Observed {
        stdout_then,
        stderr_then,
        stdout: text(&stdout),
    }
}

#[test]
fn the_event_is_sent_after_the_command_has_written_its_output() {
    let scratch = Scratch::with_reading_list();
    seen(&scratch);
    let collector = Collector::silent();
    let mut command = scratch.command(None, &Clock::fixed(EPOCH));
    sending(&mut command, &collector.url()).args(["find", "shelf"]);
    let observed = observe(command, || collector.requests().len());
    assert_ne!(observed.stdout, "");
    assert_eq!(observed.stdout_then, observed.stdout);
}

#[test]
fn the_notice_is_printed_before_the_event_is_sent() {
    let scratch = Scratch::with_reading_list();
    let collector = Collector::silent();
    let mut command = scratch.command(None, &Clock::fixed(EPOCH));
    sending(&mut command, &collector.url()).arg("check");
    let observed = observe(command, || collector.requests().len());
    assert!(
        observed.stderr_then.contains("telemetry is on"),
        "{}",
        observed.stderr_then
    );
}

#[test]
fn github_is_asked_after_the_command_has_written_its_output() {
    let scratch = Scratch::with_reading_list();
    let feed = ReleaseFeed::silent();
    let mut command = scratch.command(None, &Clock::fixed(EPOCH));
    asking(&mut command, &feed.url()).args(["find", "shelf"]);
    let observed = observe(command, || feed.requests().len());
    assert_ne!(observed.stdout, "");
    assert_eq!(observed.stdout_then, observed.stdout);
}
