//! The browser simulator's executable: visits the address in its one argument as the scenario
//! that `SOFTWARE_PLANT_BROWSER` names says.

use std::process::ExitCode;

use software_plant::browser::{SCENARIO_ENV, run};

fn main() -> ExitCode {
    let address = std::env::args().nth(1).unwrap_or_default();
    let code = std::env::var_os(SCENARIO_ENV).map_or(1, |dir| run(dir.as_ref(), &address));
    ExitCode::from(code)
}
