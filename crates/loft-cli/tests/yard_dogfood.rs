//! The developer launcher's restart policy, with a simulated clock in the Python tests.

use std::fs;
use std::process::Command;

use serde_json::{Value, json};

use crate::common::EPOCH;
use crate::common::workspace;
use crate::yard::Yard;
use software_plant::Clock;

#[test]
fn the_dogfood_restart_policy_replaces_through_serve_replace_with_bounded_waits() {
    let status = Command::new("python3")
        .arg(workspace().join("crates/loft-cli/tests/yard_dogfood_test.py"))
        .status()
        .expect("Python 3 runs the restart policy tests");
    assert!(status.success(), "restart policy tests failed: {status}");
}

#[test]
fn the_dogfood_launch_paths_restart_before_starting_either_agent() {
    let status = Command::new("python3")
        .arg(workspace().join("crates/loft-cli/tests/yard_dogfood_launch_test.py"))
        .status()
        .expect("Python 3 runs the isolated host launch tests");
    assert!(status.success(), "host launch tests failed: {status}");
}

#[test]
fn the_dogfood_restart_contract_replaces_one_server_and_preserves_recorded_state() {
    let yard = Yard::new();
    yard.write(
        "s1",
        r#"[{"id":"build","kind":"stage","title":"Build","status":"active"},
            {"id":"result","kind":"note","stage":"build","markdown":"A saved result."}]"#,
        &[],
    );
    let clock = Clock::fixed(EPOCH);
    let driver = yard.scratch.command(Some(&yard.git), &clock);
    let env: serde_json::Map<String, Value> = driver
        .get_envs()
        .map(|(key, value)| {
            (
                key.to_string_lossy().into_owned(),
                value.map_or(Value::Null, |value| json!(value.to_string_lossy())),
            )
        })
        .collect();
    let setup = yard.scratch.home().join("dogfood-contract.json");
    fs::write(
        &setup,
        json!({
            "program": driver.get_program().to_string_lossy(),
            "cwd": yard.scratch.0.canonicalize().unwrap(),
            "env": env,
        })
        .to_string(),
    )
    .unwrap();
    let status = Command::new("python3")
        .arg(workspace().join("crates/loft-cli/tests/yard_dogfood_contract.py"))
        .arg(&setup)
        .status()
        .expect("Python 3 runs the local server contract");
    assert!(status.success(), "local server contract failed: {status}");
}
