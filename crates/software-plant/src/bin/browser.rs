//! The browser simulator's executable: visits the address in its one argument as the scenario
//! that `SOFTWARE_PLANT_BROWSER` names says, and refuses any other number of arguments.

use std::process::ExitCode;

use software_plant::browser::{SCENARIO_ENV, run};

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let code = match std::env::var_os(SCENARIO_ENV) {
        Some(dir) => run(dir.as_ref(), &arguments),
        None if arguments.len() != 1 => 2,
        None => 1,
    };
    ExitCode::from(code)
}
