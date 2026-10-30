//! The browser port: opens an address in the person's web browser, through the program that
//! `BROWSER` names, given the address as its one argument, or else the system's own opener of
//! addresses. The browser is started and left to run; whether it could open is learnt from its
//! exit while the sign-in waits.

use std::process::{Child, Command, Stdio};

use crate::environment;

/// Starts the browser at `address`, or none where no program could be started.
pub fn open(address: &str) -> Option<Child> {
    let mut command = environment::value("BROWSER").map_or_else(system, Command::new);
    command
        .arg(address)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()
}

/// The system's own opener of addresses.
fn system() -> Command {
    if cfg!(target_os = "macos") {
        Command::new("open")
    } else if cfg!(windows) {
        let mut command = Command::new("rundll32");
        command.arg("url.dll,FileProtocolHandler");
        command
    } else {
        Command::new("xdg-open")
    }
}
