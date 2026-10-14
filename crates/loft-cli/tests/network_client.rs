//! The network client is the only part of `loft` that opens a network connection: no other
//! source file of the command line names the HTTP library or a socket. It goes through the
//! proxy the environment names.

use std::fs;
use std::path::Path;

use crate::common::telemetry::{asking, seen, sending};
use crate::common::{CLOSED, EPOCH, Scratch};
use software_plant::{Clock, Collector, ReleaseFeed};

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
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    for built_in in ["loft-cli", "loftspec-store"] {
        sources(&crates.join(built_in).join("src"), &mut found);
    }
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

/// A stand-in proxy: a server that records each request and answers every one with an error.
fn proxy() -> ReleaseFeed {
    ReleaseFeed::failing(502)
}

/// Runs `loft check` with the update check on, asking a closed port, with `variables` set.
fn through(variables: &[(&str, &str)]) {
    let scratch = Scratch::with_reading_list();
    let mut command = scratch.command(None, &Clock::fixed(EPOCH));
    asking(&mut command, CLOSED)
        .envs(variables.iter().copied())
        .arg("check")
        .output()
        .unwrap();
}

#[test]
fn a_named_proxy_carries_the_request() {
    for name in ["ALL_PROXY", "HTTPS_PROXY", "HTTP_PROXY"] {
        let proxy = proxy();
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
    for (earlier, later) in [
        ("ALL_PROXY", "HTTPS_PROXY"),
        ("ALL_PROXY", "HTTP_PROXY"),
        ("HTTPS_PROXY", "HTTP_PROXY"),
    ] {
        let (first, second) = (proxy(), proxy());
        through(&[(earlier, &first.url()), (later, &second.url())]);
        assert_eq!(first.requests().len(), 1, "{earlier} before {later}");
        assert!(second.requests().is_empty(), "{earlier} before {later}");
    }
}

#[test]
fn a_named_collector_is_the_only_one_contacted() {
    let scratch = Scratch::with_reading_list();
    seen(&scratch);
    let collector = Collector::accepting();
    let proxy = proxy();
    let mut command = scratch.command(None, &Clock::fixed(EPOCH));
    sending(&mut command, &collector.url())
        .env("HTTPS_PROXY", proxy.url())
        .env("NO_PROXY", "127.0.0.1")
        .arg("check")
        .output()
        .unwrap();
    assert_eq!(collector.events().len(), 1);
    assert!(proxy.requests().is_empty(), "{:?}", proxy.requests());
}

#[test]
fn a_host_that_no_proxy_lists_is_reached_directly() {
    let proxy = proxy();
    through(&[("ALL_PROXY", &proxy.url()), ("NO_PROXY", "127.0.0.1")]);
    assert!(proxy.requests().is_empty());
}
