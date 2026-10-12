//! The git simulator's executable: answers the call in its arguments from the scenario that
//! `SOFTWARE_PLANT_GIT_SCENARIO` names, and records beside the scenario each lock the call takes
//! as git would take it.

use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use software_plant::git::{GitScenario, LOCKS_FILE, OPTIONAL_LOCKS_ENV, SCENARIO_ENV, UNSCRIPTED};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let (code, stdout, stderr) = match GitScenario::from_env() {
        Ok(scenario) => {
            let reply = scenario.answer(&args);
            let optional_locks = std::env::var_os(OPTIONAL_LOCKS_ENV);
            if let Some(lock) = reply.optional_lock
                && optional_locks.as_deref() != Some("0".as_ref())
            {
                record(lock);
            }
            (reply.code, reply.stdout, reply.stderr)
        }
        Err(error) => (UNSCRIPTED, Vec::new(), format!("software-plant: {error}\n")),
    };
    let _ = io::stdout().write_all(&stdout);
    let _ = io::stderr().write_all(stderr.as_bytes());
    ExitCode::from(u8::try_from(code).unwrap_or(1))
}

/// Appends `lock` to the record beside the scenario.
fn record(lock: &str) {
    let scenario = std::env::var_os(SCENARIO_ENV).expect("the scenario was read from it");
    let path = Path::new(&scenario).with_file_name(LOCKS_FILE);
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .expect("the record of locks opens");
    writeln!(file, "{lock}").expect("the lock is recorded");
}
