//! The shell alias with which `loft merge-driver --install` runs `<command> --version` as git
//! runs the merge driver: through the shell, from the top of the working tree. The scenario
//! gives each command that runs the version it prints; any other command is not found.

use std::collections::BTreeMap;

use super::Reply;

/// The alias's name.
pub const ALIAS: &str = "loft-launcher";

/// The command a `-c alias.loft-launcher=!<command> --version loft-launcher` call runs.
pub(super) fn parse<'a>(rest: &[&'a str]) -> Option<&'a str> {
    let ["-c", alias, name] = rest else {
        return None;
    };
    if *name != ALIAS {
        return None;
    }
    alias
        .strip_prefix("alias.loft-launcher=!")?
        .strip_suffix(" --version")
}

/// What the shell prints for `command --version`: the version the scenario gives the command,
/// or the shell's exit status for a command it does not find.
pub(super) fn answer(launchers: &BTreeMap<String, String>, command: &str) -> Reply {
    launchers.get(command).map_or_else(
        || Reply::fail(127, format!("{command} --version: command not found\n")),
        |version| Reply::ok(version.clone()),
    )
}
