use std::fs;
use std::net::{IpAddr, Ipv6Addr, SocketAddr, TcpListener, TcpStream};
use std::time::Duration;

use serde_json::{Value, json};
use yardlog_store::lock;

use crate::served::{LOOPBACK, Served, first_document, free_port, machine_address, within};
use crate::yard::Yard;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Runs `loft yard serve` and reads its first document within 10 seconds, ending it where it
/// serves instead of exiting, so that a page server it should have left running fails the test
/// rather than hanging it.
fn serve_once(yard: &Yard, args: &[&str]) -> (Value, Option<i32>) {
    let mut child = yard.spawn(args);
    let document = first_document(&mut child);
    if document["outcome"] != "running" {
        let _ = child.kill();
    }
    (document, child.wait().unwrap().code())
}

#[test]
fn without_listen_a_connection_to_another_address_of_the_machine_is_not_accepted() {
    let yard = Yard::new();
    let served = Served::start(&yard, &[]);
    assert!(served.takes_at(LOOPBACK));
    assert!(!served.takes_at(machine_address()));
}

#[test]
fn with_listen_on_every_address_the_page_opens_from_another_machine() {
    let yard = Yard::new();
    let served = Served::start(&yard, &["--listen", "0.0.0.0"]);
    assert_eq!(served.version_at(machine_address()).status, 200);
    assert_eq!(served.version_at(LOOPBACK).status, 200);
}

#[test]
fn listen_serves_on_the_address_it_names_and_on_the_loopback_interface() {
    let yard = Yard::new();
    let machine = machine_address();
    let served = Served::start(&yard, &["--listen", &machine.to_string()]);
    assert_eq!(served.version_at(machine).status, 200);
    assert_eq!(served.version_at(LOOPBACK).status, 200);
}

#[test]
fn listen_may_be_given_more_than_once_and_serves_on_each_address() {
    let yard = Yard::new();
    let machine = machine_address();
    let ipv6_loopback = IpAddr::V6(Ipv6Addr::LOCALHOST);
    let served = Served::start(
        &yard,
        &["--listen", &machine.to_string(), "--listen", "::1"],
    );
    assert_eq!(served.version_at(machine).status, 200);
    assert_eq!(served.version_at(ipv6_loopback).status, 200);
    assert_eq!(served.version_at(LOOPBACK).status, 200);
}

#[test]
fn with_listen_a_connection_to_an_address_it_does_not_name_is_not_accepted() {
    let yard = Yard::new();
    let served = Served::start(&yard, &["--listen", "::1"]);
    assert!(served.takes_at(IpAddr::V6(Ipv6Addr::LOCALHOST)));
    assert!(!served.takes_at(machine_address()));
}

#[test]
fn an_address_the_machine_lacks_leaves_the_page_served_on_the_loopback_interface() {
    let yard = Yard::new();
    let mut served = Served::start(&yard, &["--listen", "192.0.2.1"]);
    assert_eq!(served.version_at(LOOPBACK).status, 200);
    assert!(
        !served.ends_within(Duration::from_millis(1_500)),
        "the page server exited for a missing address"
    );
    assert_eq!(served.version_at(LOOPBACK).status, 200);
}

#[test]
fn listen_naming_something_other_than_an_ip_address_exits_2_and_names_it() {
    let yard = Yard::new();
    let output = yard.run(&["serve", "--listen", "machine.example.ts.net"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("machine.example.ts.net"), "{stderr}");
    assert!(!yard.scratch.home().join(".yard/server.json").exists());
}

#[test]
fn listen_naming_the_unspecified_ipv6_address_in_any_form_exits_2_pointing_to_every_ipv4_address() {
    let yard = Yard::new();
    for unspecified in ["::", "::0", "0:0:0:0:0:0:0:0", "::ffff:0.0.0.0"] {
        let port = free_port().to_string();
        let mut child = yard.spawn(&["serve", "--port", &port, "--listen", unspecified]);
        let ended = within(Duration::from_secs(10), || {
            child.try_wait().unwrap().is_some()
        });
        if !ended {
            let _ = child.kill();
        }
        let output = child.wait_with_output().unwrap();
        assert!(ended, "{unspecified} was served");
        assert_eq!(output.status.code(), Some(2), "{unspecified}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(unspecified), "{stderr}");
        assert!(stderr.contains("name `0.0.0.0`"), "{stderr}");
    }
    assert!(!yard.scratch.home().join(".yard/server.json").exists());
}

#[test]
fn serve_without_listen_replaces_a_page_server_listening_on_every_address() {
    let yard = Yard::new();
    let mut every = Served::start(&yard, &["--listen", "0.0.0.0"]);
    let loopback = Served::start(&yard, &[]);
    assert!(every.ends_within(Duration::from_secs(1)));
    let named = yard.home_file("server.json");
    assert_eq!(named["port"], loopback.port);
    assert_eq!(named["listen"], json!([]));
    assert!(!loopback.takes_at(machine_address()));
}

#[test]
fn serve_naming_the_running_page_servers_addresses_in_another_order_exits_0_and_starts_none() {
    let yard = Yard::new();
    let machine = machine_address().to_string();
    let mut served = Served::start(&yard, &["--listen", &machine, "--listen", "::1"]);
    let port = free_port().to_string();
    let (running, code) = serve_once(
        &yard,
        &[
            "serve", "--port", &port, "--listen", "::1", "--listen", &machine,
        ],
    );
    assert_eq!(running["outcome"], "running", "{running}");
    assert_eq!(code, Some(0));
    assert_eq!(running["port"], served.port);
    assert!(!served.ends_within(Duration::from_millis(500)));
}

#[test]
fn a_page_server_of_a_newer_version_is_not_replaced_whatever_addresses_it_listens_on() {
    let yard = Yard::new();
    let dir = yard.scratch.home().join(".yard");
    fs::create_dir_all(&dir).unwrap();
    let port = free_port();
    let newer = json!({ "port": port, "pid": 1, "version": "99.0.0", "listen": ["0.0.0.0"] });
    fs::write(dir.join("server.json"), newer.to_string()).unwrap();
    let _held = lock::lock(&dir.join("server.lock")).unwrap();
    let output = yard.run(&["serve", "--port", &free_port().to_string()]);
    assert_eq!(output.status.code(), Some(0));
    let running: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(running["outcome"], "running", "{running}");
    assert_eq!(running["version"], "99.0.0");
    assert!(!dir.join("stop.json").exists());
}

#[test]
fn serve_join_leaves_a_page_server_of_its_version_on_other_addresses_serving() {
    let yard = Yard::new();
    let mut every = Served::start(&yard, &["--listen", "0.0.0.0"]);
    let port = free_port().to_string();
    let (running, code) = serve_once(&yard, &["serve", "--port", &port, "--join"]);
    assert_eq!(running["outcome"], "running", "{running}");
    assert_eq!(code, Some(0));
    assert_eq!(running["port"], every.port);
    assert!(!every.ends_within(Duration::from_millis(500)));
    assert_eq!(yard.home_file("server.json")["listen"], json!(["0.0.0.0"]));
}

#[test]
fn serve_join_still_asks_a_page_server_of_an_older_version_to_stop_and_serves_in_its_place() {
    let yard = Yard::new();
    let dir = yard.scratch.home().join(".yard");
    fs::create_dir_all(&dir).unwrap();
    let port = free_port();
    let older = json!({ "port": port, "pid": 1, "version": "0.0.1" });
    fs::write(dir.join("server.json"), older.to_string()).unwrap();
    let held = lock::lock(&dir.join("server.lock")).unwrap();
    let mut joining = yard.spawn(&["serve", "--port", &port.to_string(), "--join"]);
    assert!(within(Duration::from_secs(5), || dir
        .join("stop.json")
        .exists()));
    drop(held);
    let started = first_document(&mut joining);
    let _ = joining.kill();
    let _ = joining.wait();
    assert_eq!(started["outcome"], "serving", "{started}");
}

#[test]
fn serve_asks_a_page_server_that_took_the_lock_meanwhile_to_stop_as_well() {
    let yard = Yard::new();
    let dir = yard.scratch.home().join(".yard");
    fs::create_dir_all(&dir).unwrap();
    let port = free_port();
    let named = |pid: u64| {
        json!({ "port": port, "pid": pid, "version": VERSION, "listen": ["0.0.0.0"] }).to_string()
    };
    fs::write(dir.join("server.json"), named(1)).unwrap();
    let held = lock::lock(&dir.join("server.lock")).unwrap();
    let mut replacing = yard.spawn(&["serve", "--port", &port.to_string()]);
    let stop = dir.join("stop.json");
    let target = || {
        fs::read(&stop)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .map(|request| request["target"].clone())
    };
    assert!(within(Duration::from_secs(5), || target() == Some(json!(1))));
    // Another page server takes the lock, names itself and removes the request as it serves.
    fs::write(dir.join("server.json"), named(2)).unwrap();
    fs::remove_file(&stop).unwrap();
    assert!(within(Duration::from_secs(5), || target() == Some(json!(2))));
    drop(held);
    let started = first_document(&mut replacing);
    let _ = replacing.kill();
    let _ = replacing.wait();
    assert_eq!(started["outcome"], "serving", "{started}");
}

#[test]
fn a_stop_request_that_names_another_page_servers_process_leaves_the_page_server_serving() {
    let yard = Yard::new();
    let mut served = Served::start(&yard, &[]);
    let stop = yard.scratch.home().join(".yard/stop.json");
    let request = |target: u32| json!({ "version": "99.0.0", "replace": true, "target": target });
    fs::write(&stop, request(served.child.id() + 1).to_string()).unwrap();
    assert!(!served.ends_within(Duration::from_millis(1_500)));
    fs::write(&stop, request(served.child.id()).to_string()).unwrap();
    assert!(served.ends_within(Duration::from_secs(1)));
}

#[test]
fn serve_join_and_replace_together_are_a_usage_error() {
    let yard = Yard::new();
    let output = yard.run(&["serve", "--join", "--replace"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(!yard.scratch.home().join(".yard/server.json").exists());
}

#[test]
fn serve_join_where_none_runs_after_a_loopback_page_server_listens_on_the_loopback_interface_alone()
{
    let yard = Yard::new();
    drop(Served::start(&yard, &[]));
    let joined = Served::start(&yard, &["--join", "--listen", "0.0.0.0"]);
    assert!(joined.takes_at(LOOPBACK));
    assert!(!joined.takes_at(machine_address()));
    assert_eq!(yard.home_file("server.json")["listen"], json!([]));
}

#[test]
fn serve_join_where_none_runs_after_a_page_server_on_every_address_listens_on_every_address() {
    let yard = Yard::new();
    drop(Served::start(&yard, &["--listen", "0.0.0.0"]));
    let joined = Served::start(&yard, &["--join"]);
    assert_eq!(joined.version_at(machine_address()).status, 200);
    assert_eq!(yard.home_file("server.json")["listen"], json!(["0.0.0.0"]));
}

#[test]
fn serve_join_where_no_addresses_were_recorded_listens_on_the_loopback_interface_alone() {
    let yard = Yard::new();
    let joined = Served::start(&yard, &["--join", "--listen", "0.0.0.0"]);
    assert!(!joined.takes_at(machine_address()));
    drop(joined);
    let earlier = json!({ "port": free_port(), "pid": 1, "version": "0.0.1" });
    fs::write(
        yard.scratch.home().join(".yard/server.json"),
        earlier.to_string(),
    )
    .unwrap();
    let joined = Served::start(&yard, &["--join", "--listen", "0.0.0.0"]);
    assert!(joined.takes_at(LOOPBACK));
    assert!(!joined.takes_at(machine_address()));
}

#[test]
fn a_start_that_does_not_serve_leaves_no_port_and_the_addresses_the_last_page_server_served_on() {
    let yard = Yard::new();
    drop(Served::start(&yard, &[]));
    let other = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = other.local_addr().unwrap().port().to_string();
    let output = yard.run(&["serve", "--port", &port, "--listen", "0.0.0.0"]);
    assert_eq!(output.status.code(), Some(1));
    let named = yard.home_file("server.json");
    assert!(named["pid"].as_u64().is_some(), "{named}");
    assert_eq!(named["version"], VERSION);
    assert_eq!(named["listen"], json!([]));
    assert!(named.get("port").is_none(), "{named}");
    let joined = Served::start(&yard, &["--join"]);
    assert!(!joined.takes_at(machine_address()));
}

#[test]
fn a_start_that_does_not_serve_keeps_every_address_the_last_page_server_served_on() {
    let yard = Yard::new();
    let served = Served::start(&yard, &["--listen", "0.0.0.0"]);
    let pid = served.child.id();
    drop(served);
    let other = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = other.local_addr().unwrap().port().to_string();
    let output = yard.run(&["serve", "--port", &port]);
    assert_eq!(output.status.code(), Some(1));
    let named = yard.home_file("server.json");
    assert_ne!(named["pid"], json!(pid), "{named}");
    assert_eq!(named["listen"], json!(["0.0.0.0"]));
    assert!(named.get("port").is_none(), "{named}");
    let joined = Served::start(&yard, &["--join"]);
    assert!(joined.takes_at(machine_address()));
}

#[test]
fn serve_join_replacing_a_page_server_of_an_older_version_listens_on_the_recorded_addresses() {
    let yard = Yard::new();
    let dir = yard.scratch.home().join(".yard");
    fs::create_dir_all(&dir).unwrap();
    let port = free_port();
    let older = json!({ "port": port, "pid": 1, "version": "0.0.1" });
    fs::write(dir.join("server.json"), older.to_string()).unwrap();
    let held = lock::lock(&dir.join("server.lock")).unwrap();
    let mut joining = yard.spawn(&[
        "serve",
        "--port",
        &port.to_string(),
        "--join",
        "--listen",
        "0.0.0.0",
    ]);
    assert!(within(Duration::from_secs(5), || dir
        .join("stop.json")
        .exists()));
    drop(held);
    let started = first_document(&mut joining);
    let reached = TcpStream::connect_timeout(
        &SocketAddr::new(machine_address(), port),
        Duration::from_secs(5),
    );
    let _ = joining.kill();
    let _ = joining.wait();
    assert_eq!(started["outcome"], "serving", "{started}");
    assert!(
        reached.is_err(),
        "the page server listened on every address"
    );
    assert_eq!(yard.home_file("server.json")["listen"], json!([]));
}

#[test]
fn serve_waits_for_a_starting_page_server_to_give_its_port_and_compares_with_its_addresses() {
    let yard = Yard::new();
    let dir = yard.scratch.home().join(".yard");
    fs::create_dir_all(&dir).unwrap();
    let port = free_port();
    let named = |record: Value| fs::write(dir.join("server.json"), record.to_string()).unwrap();
    let held = lock::lock(&dir.join("server.lock")).unwrap();
    named(json!({ "pid": 2, "version": VERSION, "listen": ["0.0.0.0"] }));
    let mut serving = yard.spawn(&["serve", "--port", &port.to_string()]);
    assert!(
        !within(Duration::from_millis(1_500), || serving
            .try_wait()
            .unwrap()
            .is_some()),
        "loft yard serve did not wait for the starting page server's port"
    );
    assert!(!dir.join("stop.json").exists());
    named(json!({ "port": port, "pid": 2, "version": VERSION, "listen": ["0.0.0.0"] }));
    let stop = dir.join("stop.json");
    assert!(within(Duration::from_secs(5), || {
        fs::read(&stop)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .is_some_and(|request| request["target"] == json!(2))
    }));
    drop(held);
    let started = first_document(&mut serving);
    let _ = serving.kill();
    let _ = serving.wait();
    assert_eq!(started["outcome"], "serving", "{started}");
}
