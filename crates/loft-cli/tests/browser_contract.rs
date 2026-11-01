//! The contract suite of the browser simulator: the simulator, and on demand the system's own
//! opener of addresses, are each given a loopback address and must request it. The real half
//! opens the person's browser, so it is ignored by default; run it with
//! `cargo nextest run -p loft-cli --run-ignored only -E 'test(/browser_contract/)'`.

use std::process::Command;
use std::time::Duration;

use software_plant::http::{Answer, Request, Responder, Response, Server};
use software_plant::{BrowserScenario, InstalledBrowser, browser_executable};

use crate::common::Scratch;
use crate::served::within;

/// Redirects `/start` to `/landing`, and answers every other path with a page.
struct Redirecting;

impl Responder for Redirecting {
    fn answer(&mut self, request: &Request) -> Answer {
        if request.path == "/start" {
            Response::redirect("/landing").into()
        } else {
            Response::empty(200).into()
        }
    }
}

/// Runs the program `BROWSER` names, as `loft` does, with `address`.
fn open_with(installed: &InstalledBrowser, address: &str) -> Option<i32> {
    let mut command = Command::new(browser_executable());
    installed.apply(&mut command);
    command.arg(address).status().unwrap().code()
}

#[test]
fn a_visiting_browser_requests_the_address_and_follows_each_redirect() {
    let scratch = Scratch::empty();
    let server = Server::start(Redirecting);
    let installed = BrowserScenario::Visiting.install(&scratch.0);
    let address = format!("{}/start", server.url());
    assert_eq!(open_with(&installed, &address), Some(0));
    let paths: Vec<String> = server.requests().into_iter().map(|r| r.path).collect();
    assert_eq!(paths, ["/start", "/landing"]);
    assert_eq!(installed.visits(), [address]);
}

#[test]
fn an_absent_browser_requests_nothing_and_exits_1() {
    let scratch = Scratch::empty();
    let server = Server::start(Redirecting);
    let installed = BrowserScenario::Absent.install(&scratch.0);
    let address = format!("{}/start", server.url());
    assert_eq!(open_with(&installed, &address), Some(1));
    assert!(server.requests().is_empty());
    assert_eq!(installed.visits(), [address]);
}

#[test]
fn a_browser_run_with_other_than_one_argument_requests_nothing_and_exits_2() {
    let scratch = Scratch::empty();
    let server = Server::start(Redirecting);
    let installed = BrowserScenario::Visiting.install(&scratch.0);
    let address = format!("{}/start", server.url());
    for arguments in [vec![], vec![address.clone(), "--new-window".to_owned()]] {
        let mut command = Command::new(browser_executable());
        installed.apply(&mut command);
        assert_eq!(command.args(&arguments).status().unwrap().code(), Some(2));
    }
    assert!(server.requests().is_empty());
    assert_eq!(installed.visits(), Vec::<String>::new());
    assert_eq!(
        installed.arguments(),
        [vec![], vec![address, "--new-window".to_owned()]]
    );
}

#[test]
#[ignore = "opens the person's browser"]
fn the_system_s_opener_requests_the_address_as_the_simulator_does() {
    let server = Server::start(Redirecting);
    let address = format!("{}/start", server.url());
    let opener = if cfg!(target_os = "macos") {
        Command::new("open").arg(&address).status()
    } else if cfg!(windows) {
        Command::new("cmd")
            .args(["/C", "start", "", &address])
            .status()
    } else {
        Command::new("xdg-open").arg(&address).status()
    };
    assert!(opener.unwrap().success());
    within(Duration::from_secs(30), || server.requests().len() >= 2);
    let paths: Vec<String> = server.requests().into_iter().map(|r| r.path).collect();
    assert_eq!(&paths[..2], ["/start", "/landing"]);
}
