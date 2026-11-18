//! Where the page server listens: on every address of the machine unless `--listen` names
//! addresses, and then on each of them and on the loopback interface; an address the machine
//! lacks leaves the page served elsewhere; and `--listen` takes IP addresses alone.

use std::net::IpAddr;
use std::time::Duration;

use crate::served::{Served, machine_address};
use crate::yard::Yard;

const LOOPBACK: IpAddr = IpAddr::V4(std::net::Ipv4Addr::LOCALHOST);

#[test]
fn without_listen_the_page_server_listens_on_every_address_of_the_machine() {
    let yard = Yard::new();
    let served = Served::start_on_every_address(&yard, &[]);
    let machine = machine_address();
    assert_eq!(served.version_at(machine).status, 200);
    assert_eq!(served.version_at(LOOPBACK).status, 200);
}

#[test]
fn listen_serves_on_the_address_it_names_and_on_the_loopback_interface() {
    let yard = Yard::new();
    let machine = machine_address().to_string();
    let served = Served::start(&yard, &["--listen", &machine]);
    assert_eq!(served.version_at(machine_address()).status, 200);
    assert_eq!(served.version_at(LOOPBACK).status, 200);
}

#[test]
fn listen_may_be_given_more_than_once_and_serves_on_each_address() {
    let yard = Yard::new();
    let machine = machine_address().to_string();
    let served = Served::start(&yard, &["--listen", &machine, "--listen", "127.0.0.1"]);
    assert_eq!(served.version_at(machine_address()).status, 200);
    assert_eq!(served.version_at(LOOPBACK).status, 200);
}

#[test]
fn with_listen_a_connection_to_an_address_it_does_not_name_is_not_accepted() {
    let yard = Yard::new();
    let served = Served::start(&yard, &["--listen", "127.0.0.1"]);
    assert!(served.takes_at(LOOPBACK));
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
