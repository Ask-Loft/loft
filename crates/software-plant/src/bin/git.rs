//! The git simulator's executable: answers the call in its arguments from the scenario that
//! `SOFTWARE_PLANT_GIT_SCENARIO` names.

use std::io::{self, Write};
use std::process::ExitCode;

use software_plant::git::{GitScenario, UNSCRIPTED};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let (code, stdout, stderr) = match GitScenario::from_env() {
        Ok(scenario) => {
            let reply = scenario.answer(&args);
            (reply.code, reply.stdout, reply.stderr)
        }
        Err(error) => (UNSCRIPTED, Vec::new(), format!("software-plant: {error}\n")),
    };
    let _ = io::stdout().write_all(&stdout);
    let _ = io::stderr().write_all(stderr.as_bytes());
    ExitCode::from(u8::try_from(code).unwrap_or(1))
}
