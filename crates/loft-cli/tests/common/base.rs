//! A base revision as the git simulator scripts it: the commit `git merge-base HEAD <revision>`
//! gives, holding a scratch tree's files as they are when the scenario is made, for the commands
//! that compare the tree with a base.

use software_plant::GitScenario;

use super::common::{Scratch, tree};

/// The commit the base revision `develop` gives.
pub const COMMIT: &str = "c0ffee00c0ffee00c0ffee00c0ffee00c0ffee00";

/// A clean repository on `feature` whose base for `develop` holds the tree as it now is.
pub fn based_on_now(scratch: &Scratch) -> GitScenario {
    let mut scenario = GitScenario::clean_on("feature").base("develop", COMMIT);
    for (number, (path, bytes)) in tree(&scratch.0).iter().enumerate() {
        let text = String::from_utf8(bytes.clone()).unwrap();
        let path = path.to_string();
        scenario = scenario.file_at("develop", &path, &format!("{number:040x}"), &text);
    }
    scenario
}
