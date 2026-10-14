//! `loft record-install <way>`: records how this executable was installed, for the update notice
//! and the command event. The npm package runs it from its install script, so a record that
//! cannot be written is told on standard error and still exits 0: an install never fails
//! because of it. It reads no tree, sends no event and asks GitHub nothing.

use serde_json::json;

use crate::finish::Finished;
use crate::install::{self, Way};
use crate::json;
use crate::out::errln;

/// The schema `--format json` writes to, by name and version.
const SCHEMA: &str = "loft-record-install/1";

pub fn run(way: Way, as_json: bool) -> Finished {
    match install::record(way) {
        Ok(path) => {
            if as_json {
                let fields = json!({ "path": path, "way": way.as_str() });
                json::print(&json::result(SCHEMA, "recorded", fields, Vec::new()));
            }
            Finished::success("recorded")
        }
        Err(error) => {
            let message = error.to_string();
            let errors = vec![json::error("settings", &message, &[])];
            let finished = Finished::with(std::process::ExitCode::SUCCESS, "unwritten", &errors);
            if as_json {
                let fields = json!({ "way": way.as_str() });
                json::print(&json::result(SCHEMA, "unwritten", fields, errors));
            }
            errln!("loft record-install: {message}");
            finished
        }
    }
}
