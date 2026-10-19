//! A base revision as the git simulator scripts it: the commit `git merge-base HEAD <revision>`
//! gives, holding a scratch tree's files as they are when the scenario is made, for the commands
//! that compare the tree with a base.

use std::fs;
use std::path::Path;

use software_plant::GitScenario;

use super::common::Scratch;

/// The commit the base revision `develop` gives.
pub const COMMIT: &str = "c0ffee00c0ffee00c0ffee00c0ffee00c0ffee00";

/// Every file under `dir`, by its path from `dir` after `prefix`.
fn files(dir: &Path, prefix: &str, into: &mut Vec<(String, String)>) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let relative = format!("{prefix}{name}");
        if path.is_dir() {
            files(&path, &format!("{relative}/"), into);
        } else {
            into.push((relative, fs::read_to_string(&path).unwrap()));
        }
    }
}

/// A clean repository on `feature` whose base for `develop` holds the tree as it now is.
pub fn based_on_now(scratch: &Scratch) -> GitScenario {
    let mut held = Vec::new();
    files(&scratch.0.join("spec"), "spec/", &mut held);
    let mut scenario = GitScenario::clean_on("feature").base("develop", COMMIT);
    for (number, (path, text)) in held.iter().enumerate() {
        scenario = scenario.file_at("develop", path, &format!("{number:040x}"), text);
    }
    scenario
}
