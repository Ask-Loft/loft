//! The `loft-spec` skill (the Agent Skill Product): its host-neutral source, embedded in `loft`,
//! composed for each agent host and written into a project by `loft skill install` and
//! `loft init`, which also enable the Claude Code plugin `loft@ask-loft` beside it unless
//! `--no-plugin` is given.

mod compose;
mod discovery;
mod hosts;
mod settings;
mod write;

use std::fmt;
use std::io;
use std::path::Path;

use serde_json::json;

use crate::finish::Finished;
use crate::json;
use crate::out::outln;

use compose::Host;
pub use hosts::Hosts;
pub use settings::Enabled;
use settings::SETTINGS;

/// The schema `--format json` writes to, by name and version (RQ-156).
const SCHEMA: &str = "loft-skill/1";

/// A skill's source, each file by its path under `skill/<skill>/`, embedded in the binary.
macro_rules! source {
    ($skill:literal: $($path:literal),* $(,)?) => {
        &[$(($path, include_str!(concat!("../../skill/", $skill, "/", $path)))),*]
    };
}

const SOURCE: &[(&str, &str)] = source![
    "loft-spec":
    "SKILL.md",
    "agents/claude/claude-host.md",
    "agents/claude/frontmatter.yaml",
    "agents/claude/route.md",
    "agents/openai.yaml",
    "references/adopting-an-openspec-project.md",
    "references/changing-a-specified-product.md",
    "references/choosing-the-next-change.md",
    "references/format.md",
    "references/lint-findings.md",
    "references/new-document.md",
    "references/publishing-and-merging.md",
    "references/reconciling.md",
    "references/reviewing.md",
    "references/the-loop.md",
    "references/writing-rules.md",
];

/// The `yard` skill's source: its shared workflow and each host's adaptation.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "`loft skill package` composes it next")
)]
const YARD_SOURCE: &[(&str, &str)] =
    source!["yard": "SKILL.md", "agents/claude.md", "agents/codex.md"];

/// A skill directory that could not be written.
#[derive(Debug)]
pub struct SkillError {
    directory: &'static str,
    error: io::Error,
}

impl fmt::Display for SkillError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { directory, error } = self;
        if *directory == SETTINGS {
            write!(formatter, "{directory}: cannot enable the plugin: {error}")
        } else {
            write!(formatter, "{directory}: cannot write the skill: {error}")
        }
    }
}

/// Whether writing the skill enables the plugin beside it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Plugin {
    Enable,
    /// `--no-plugin`: `.claude/settings.json` is neither read nor written.
    Leave,
}

/// What writing the skill wrote: the skill directories, and what it did to the settings, if it
/// enabled the plugin.
pub struct Installed {
    pub skills: Vec<&'static str>,
    pub preserved: Vec<&'static str>,
    pub settings: Option<Enabled>,
}

/// Writes each host's package into its directory under `root`, one after the other, then
/// enables the plugin in `.claude/settings.json` where `plugin` says so, and names what it
/// wrote.
///
/// # Errors
///
/// A directory or the settings file that cannot be written; one not yet reached is left as it
/// was.
pub fn install(root: &Path, plugin: Plugin) -> Result<Installed, SkillError> {
    install_for(root, Hosts::All, plugin)
}

/// Installs only the selected hosts, preserving the repository's live source link.
fn install_for(root: &Path, hosts: Hosts, plugin: Plugin) -> Result<Installed, SkillError> {
    let destinations = hosts
        .selected()
        .iter()
        .map(|&host| {
            discovery::preserved(root, host)
                .map(|preserved| (host, preserved))
                .map_err(|error| SkillError {
                    directory: host.directory(),
                    error,
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut written = Vec::new();
    let mut preserved = Vec::new();
    for (host, linked) in destinations {
        if linked {
            preserved.push(host.directory());
            continue;
        }
        // The source is embedded, and its tests compose it for every host.
        let files = compose::compose(host, SOURCE).expect("the embedded skill composes");
        let directory = host.directory();
        write::replace(&root.join(directory), &files)
            .map_err(|error| SkillError { directory, error })?;
        written.push(directory);
    }
    let settings = match (plugin, hosts.includes_claude()) {
        (Plugin::Enable, true) => Some(settings::enable(root).map_err(|error| SkillError {
            directory: SETTINGS,
            error,
        })?),
        _ => None,
    };
    Ok(Installed {
        skills: written,
        preserved,
        settings,
    })
}

/// Whether each host's package of the skill lies under `root`.
pub fn installed(root: &Path) -> bool {
    Host::ALL
        .iter()
        .all(|host| root.join(host.directory()).join("SKILL.md").is_file())
}

/// An error of the skill as JSON, of kind `install`.
pub fn error_json(error: &SkillError) -> serde_json::Value {
    json::error("install", &error.to_string(), &[])
}

/// Prints the line for each skill directory written and for the settings, then asks for what
/// was written to be committed (Writing the skill).
pub fn report(installed: &Installed) {
    for directory in &installed.preserved {
        outln!("unchanged {directory} (repository source link)");
    }
    for directory in &installed.skills {
        outln!("installed {directory}");
    }
    let mut written: Vec<&str> = installed.skills.clone();
    match installed.settings {
        Some(Enabled::Written) => {
            outln!(
                "enabled the Claude Code plugin {} in {SETTINGS}",
                settings::PLUGIN
            );
            written.push(SETTINGS);
        }
        Some(Enabled::Unchanged) | None => {}
        Some(Enabled::NotAnObject) => outln!("{}", settings::by_hand()),
    }
    if !installed.skills.is_empty() {
        let last = written.pop().expect("a skill directory was written");
        let listed = if written.is_empty() {
            last.to_owned()
        } else {
            format!("{} and {last}", written.join(", "))
        };
        outln!("commit {listed}, so that every clone's coding agents have the skill");
    }
}

/// `loft skill install`: writes the skill into the current directory and names the directories
/// it wrote.
pub fn run(root: &Path, hosts: Hosts, plugin: Plugin, as_json: bool) -> Finished {
    match install_for(root, hosts, plugin) {
        Ok(installed) => {
            if as_json {
                let fields = json!({ "skills": installed.skills });
                json::print(&json::result(SCHEMA, "installed", fields, Vec::new()));
            } else {
                report(&installed);
            }
            Finished::success("installed")
        }
        Err(error) => json::stop(SCHEMA, vec![error_json(&error)], &error, as_json),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::YARD_SOURCE;
    use super::compose::compose_yard;

    #[test]
    fn the_embedded_yard_source_composes_each_committed_yard_package_byte_for_byte() {
        for (plugin, adaptation) in [("claude", "agents/claude.md"), ("codex", "agents/codex.md")] {
            let committed = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(format!("../../plugins/{plugin}/loft/skills/yard/SKILL.md"));
            assert_eq!(
                compose_yard(adaptation, YARD_SOURCE),
                Ok(vec![(
                    "SKILL.md".to_owned(),
                    fs::read_to_string(committed).unwrap()
                )]),
                "{plugin}"
            );
        }
    }
}
