//! A real Codex session with an isolated home and a loopback model fixture; no API account.

use std::process::Command;

use software_plant::real_git;
use yardlog_store::lock;

use crate::common::{isolate, stderr, stdout, workspace};
use crate::repo::Repo;

#[test]
#[ignore = "requires Codex 0.164.0 and Python 3; uses a loopback model fixture"]
fn real_codex_hooks_record_native_tool_activity_without_a_remote_model() {
    let repo = Repo::new();
    repo.git_ok(&["checkout", "--quiet", "-b", "feat/codex-contract"]);
    let mut command = Command::new("python3");
    isolate(&mut command, &repo.0.home());
    real_git::isolate(&mut command);
    let output = command
        .arg(workspace().join("crates/loft-cli/tests/codex_contract.py"))
        .arg(env!("CARGO_BIN_EXE_loft"))
        .arg(&repo.0.0)
        .arg(repo.0.home())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        stdout(&output),
        stderr(&output)
    );
    let server = repo.0.home().join(".yard/server.lock");
    assert!(
        crate::served::within(std::time::Duration::from_secs(3), || !server.exists()
            || !lock::is_held(&server).unwrap()),
        "Codex left its page-server child running"
    );
}
