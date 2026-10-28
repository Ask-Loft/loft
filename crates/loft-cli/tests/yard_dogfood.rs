//! The developer launcher's restart policy, with a simulated clock in the Python tests.

use std::process::Command;

use crate::common::workspace;

#[test]
fn the_dogfood_restart_policy_checks_process_identity_and_bounded_waits() {
    let status = Command::new("python3")
        .arg(workspace().join("crates/loft-cli/tests/yard_dogfood_test.py"))
        .status()
        .expect("Python 3 runs the restart policy tests");
    assert!(status.success(), "restart policy tests failed: {status}");
}
