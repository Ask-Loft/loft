//! The network client is the only part of `loft` that opens a network connection: no other
//! source file of the command line names the HTTP library or a socket. It goes through the
//! proxy the environment names.

use std::fs;
use std::path::Path;

use crate::common::{CLOSED, EPOCH, Scratch};
use software_plant::Clock;
use software_plant::http::{Request, Responder, Response, Server};

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
fn only_the_network_client_opens_a_connection() {
    let mut found = Vec::new();
    sources(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut found,
    );
    assert!(found.len() > 10);
    for (path, text) in found {
        if path.ends_with("network.rs") {
            assert!(text.contains("ureq::"), "{path}");
            continue;
        }
        for name in ["ureq", "TcpStream", "UdpSocket", "std::net::"] {
            assert!(!text.contains(name), "{path} names {name}");
        }
    }
}

/// A stand-in proxy: it records each request and answers none of them usefully.
struct Proxy;

impl Responder for Proxy {
    fn answer(&mut self, _: &Request) -> Option<Response> {
        Some(Response::empty(502))
    }
}

/// Runs `loft check` with the update check on, asking a closed port, with `variables` set.
fn through(variables: &[(&str, &str)]) {
    let scratch = Scratch::with_reading_list();
    let mut command = scratch.command(None, &Clock::fixed(EPOCH));
    command
        .env_remove("LOFT_UPDATE_CHECK")
        .env("LOFT_UPDATE_ENDPOINT", CLOSED);
    for (name, value) in variables {
        command.env(name, value);
    }
    command.arg("check").output().unwrap();
}

#[test]
fn a_named_proxy_carries_the_request() {
    for name in ["ALL_PROXY", "HTTPS_PROXY", "HTTP_PROXY"] {
        let proxy = Server::start(Proxy);
        through(&[(name, &proxy.url())]);
        let requests = proxy.requests();
        assert_eq!(requests.len(), 1, "{name}");
        assert!(
            requests[0].path.contains("127.0.0.1:9"),
            "{name}: {:?}",
            requests[0]
        );
    }
}

#[test]
fn the_first_proxy_variable_set_wins() {
    let (first, later) = (Server::start(Proxy), Server::start(Proxy));
    through(&[("ALL_PROXY", &first.url()), ("HTTP_PROXY", &later.url())]);
    assert_eq!(first.requests().len(), 1);
    assert!(later.requests().is_empty());
}

#[test]
fn a_host_that_no_proxy_lists_is_reached_directly() {
    let proxy = Server::start(Proxy);
    through(&[("ALL_PROXY", &proxy.url()), ("NO_PROXY", "127.0.0.1")]);
    assert!(proxy.requests().is_empty());
}
