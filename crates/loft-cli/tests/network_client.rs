//! The network client is the only part of `loft` that opens a network connection: no other
//! source file of the command line names the HTTP library or a socket.

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
