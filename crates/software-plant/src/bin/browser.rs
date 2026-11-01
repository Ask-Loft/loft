//! The browser simulator's executable: visits the address in its one argument as the scenario
//! that `SOFTWARE_PLANT_BROWSER` names says, and refuses any other number of arguments.

use std::process::ExitCode;

use software_plant::browser::{SCENARIO_ENV, run};

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let code = std::env::var_os(SCENARIO_ENV).map_or(1, |dir| run(dir.as_ref(), &arguments));
    ExitCode::from(code)
}
