use std::cell::Cell;
use std::fs;
use std::time::Duration;

use loft_ports::PortError;
use loft_ports::clock::Clock;
use software_plant::YardServer;
use yardlog_store::outbox::Outbox;

use super::sending::{MOST_TIME, drain};

const WAIT: Duration = Duration::from_secs(2);

/// A clock whose every reading of the elapsed time finds `step` more passed than the last.
struct Stepping {
    step: Duration,
    passed: Cell<Duration>,
}

impl Clock for Stepping {
    fn now(&self) -> Result<u64, PortError> {
        Ok(0)
    }

    fn elapsed(&self) -> Duration {
        let passed = self.passed.get();
        self.passed.set(passed + self.step);
        passed
    }

    fn wait(&self, _length: Duration) {}
}

/// An outbox for `endpoint` in a fresh directory, holding `count` small entries.
fn filled(endpoint: &str, count: usize) -> Outbox {
    let dir = std::env::temp_dir().join(format!("loft-flush-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    for n in 0..count {
        let key = format!("00000000-0000-7000-8000-{n:012}");
        let record = format!(
            r#"{{"record":"version","yardLog":"y","kind":"note","id":"n{n}","version":{{"key":"{key}"}}}}"#
        );
        fs::write(
            dir.join(format!("{key}.json")),
            yardlog::entry::write(endpoint, &record),
        )
        .unwrap();
    }
    Outbox::new(dir, endpoint)
}

#[test]
fn a_run_that_has_sent_for_5_minutes_ends_after_the_batch_on_its_way_and_counts_the_rest() {
    let server = YardServer::accepting();
    let outbox = filled(&server.url(), 40);
    // The run reads the clock as it begins and before each batch: at 0, 2 and 4 minutes it takes
    // a batch, and at 6 minutes, past the 5, it takes no more.
    let clock = Stepping {
        step: MOST_TIME / 5 * 2,
        passed: Cell::new(Duration::ZERO),
    };
    let flushed = drain(&outbox, software_plant::yard_server::TOKEN, WAIT, &clock);
    assert_eq!(server.requests().len(), 2);
    assert_eq!(flushed.outcome, "sent");
    assert_eq!((flushed.sent, flushed.remaining), (32, 8));
    fs::remove_dir_all(outbox.dir()).unwrap();
}

#[test]
fn a_run_with_time_to_spare_sends_every_batch() {
    let server = YardServer::accepting();
    let outbox = filled(&server.url(), 40);
    let clock = Stepping {
        step: Duration::from_secs(1),
        passed: Cell::new(Duration::ZERO),
    };
    let flushed = drain(&outbox, software_plant::yard_server::TOKEN, WAIT, &clock);
    assert_eq!(server.requests().len(), 3);
    assert_eq!((flushed.sent, flushed.remaining), (40, 0));
    fs::remove_dir_all(outbox.dir()).unwrap();
}
