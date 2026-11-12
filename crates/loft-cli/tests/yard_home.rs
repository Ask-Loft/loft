//! `LOFT_YARD_HOME`: what belongs to the machine kept in the directory it names in place of
//! `~/.yard/`, so that a page server kept there runs beside one kept elsewhere; an empty or
//! relative value is ignored.

use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::time::Duration;

use serde_json::Value;
use software_plant::Clock;

use crate::common::{EPOCH, Scratch};
use crate::served::{first_document, free_port, within};

/// A page server started by `loft yard serve` on `port`, with `LOFT_YARD_HOME` set to `home`
/// where given; killed when dropped.
struct Server {
    child: Child,
    started: Value,
}

impl Server {
    fn start(scratch: &Scratch, port: u16, home: Option<&str>) -> Self {
        let mut command = scratch.command(None, &Clock::fixed(EPOCH));
        if let Some(home) = home {
            command.env("LOFT_YARD_HOME", home);
        }
        let mut child = command
            .args([
                "yard",
                "serve",
                "--port",
                &port.to_string(),
                "--format",
                "json",
            ])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let started = first_document(&mut child);
        Self { child, started }
    }

    fn is_running(&mut self) -> bool {
        self.child.try_wait().unwrap().is_none()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn answers(port: u16) -> bool {
    TcpStream::connect(("127.0.0.1", port)).is_ok()
}

fn files_of(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

fn moved(scratch: &Scratch, name: &str) -> PathBuf {
    scratch.home().join("elsewhere").join(name)
}

#[test]
fn a_moved_home_holds_the_files_of_the_machine_and_nothing_is_written_to_the_default_one() {
    let scratch = Scratch::empty();
    let home = moved(&scratch, "yard-dev");
    let server = Server::start(&scratch, free_port(), Some(home.to_str().unwrap()));
    assert_eq!(server.started["outcome"], "serving", "{}", server.started);
    let files = files_of(&home);
    for name in ["server.json", "server.lock", "token"] {
        assert!(files.contains(&name.to_string()), "{name} in {files:?}");
    }
    assert!(
        !scratch.home().join(".yard").exists(),
        "{:?}",
        files_of(&scratch.home().join(".yard"))
    );
}

#[test]
fn page_servers_kept_in_two_homes_serve_beside_each_other() {
    let scratch = Scratch::empty();
    let (first_port, second_port) = (free_port(), free_port());
    let mut first = Server::start(&scratch, first_port, None);
    assert_eq!(first.started["outcome"], "serving", "{}", first.started);
    let home = moved(&scratch, "yard-dev");
    let mut second = Server::start(&scratch, second_port, Some(home.to_str().unwrap()));
    assert_eq!(second.started["outcome"], "serving", "{}", second.started);
    assert!(
        !within(Duration::from_secs(2), || !first.is_running()),
        "the first page server stopped"
    );
    assert!(first.is_running() && second.is_running());
    assert!(answers(first_port) && answers(second_port));
    assert!(!scratch.home().join(".yard/stop.json").exists());
    assert!(!home.join("stop.json").exists());
}

#[test]
fn an_empty_or_relative_yard_home_is_ignored_and_the_default_one_is_used() {
    for value in ["", "relative/yard"] {
        let scratch = Scratch::empty();
        let server = Server::start(&scratch, free_port(), Some(value));
        assert_eq!(server.started["outcome"], "serving", "{value:?}");
        assert!(
            scratch.home().join(".yard/server.json").exists(),
            "{value:?}: {:?}",
            files_of(&scratch.home().join(".yard"))
        );
        assert!(!scratch.0.join("relative").exists(), "{value:?}");
    }
}
