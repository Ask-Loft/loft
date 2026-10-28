//! `loft skill package`: the maintainer's command that writes every plugin package under
//! `plugins/` from the skill sources `loft` embeds, each host's `loft-spec` package and, for a
//! host with a Yard adapter, its `yard` package; or, with `--check`, writes nothing and names
//! each package file that differs from its composition. It runs at the root of the repository of
//! `loft`, and refuses while the repository's Codex discovery entry is not the link to the
//! canonical skill source.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use serde_json::json;

use super::compose::{self, Files, Host};
use super::{SOURCE, YARD_SOURCE, discovery, write};
use crate::finish::Finished;
use crate::json;
use crate::out::outln;

/// The schema `--format json` writes to, by name and version.
const SCHEMA: &str = "loft-skill-package/1";

/// A host's plugin: its root from the repository's root, and the host's adaptation of the `yard`
/// skill if the host has a Yard adapter.
struct Plugin {
    host: Host,
    root: &'static str,
    yard: Option<&'static str>,
}

const PLUGINS: [Plugin; 2] = [
    Plugin {
        host: Host::ClaudeCode,
        root: "plugins/claude/loft",
        yard: Some("agents/claude.md"),
    },
    Plugin {
        host: Host::Codex,
        root: "plugins/codex/loft",
        yard: Some("agents/codex.md"),
    },
];

/// A package directory, from the repository's root, and the files it holds: none where its host
/// gets no such package, so that the directory does not exist.
struct Package {
    directory: String,
    files: Option<Files>,
}

/// Each plugin's packages, composed from the embedded sources.
fn packages(plugins: &[Plugin]) -> Vec<Package> {
    plugins
        .iter()
        .flat_map(|plugin| {
            // The sources are embedded, and their tests compose them for every host.
            let spec = compose::compose(plugin.host, SOURCE).expect("the embedded skill composes");
            let yard = plugin.yard.map(|adaptation| {
                compose::compose_yard(adaptation, YARD_SOURCE).expect("the embedded yard composes")
            });
            [
                Package {
                    directory: format!("{}/skills/loft-spec", plugin.root),
                    files: Some(spec),
                },
                Package {
                    directory: format!("{}/skills/yard", plugin.root),
                    files: yard,
                },
            ]
        })
        .collect()
}

/// The package files that differ from their composition, each by its path from the
/// repository's root.
#[derive(Debug, Default, PartialEq, Eq)]
struct Drift {
    changed: Vec<String>,
    missing: Vec<String>,
    extra: Vec<String>,
}

impl Drift {
    fn is_empty(&self) -> bool {
        self.changed.is_empty() && self.missing.is_empty() && self.extra.is_empty()
    }
}

/// How each package under `root` differs from its composition.
fn drift(root: &Path, packages: &[Package]) -> io::Result<Drift> {
    let mut drift = Drift::default();
    for package in packages {
        let shown = |path: &str| format!("{}/{path}", package.directory);
        let mut found = files_under(&root.join(&package.directory))?;
        for (path, text) in package.files.iter().flatten() {
            match found.remove(path) {
                None => drift.missing.push(shown(path)),
                Some(bytes) if bytes != text.as_bytes() => drift.changed.push(shown(path)),
                Some(_) => {}
            }
        }
        drift.extra.extend(found.keys().map(|path| shown(path)));
    }
    for list in [&mut drift.changed, &mut drift.missing, &mut drift.extra] {
        list.sort();
    }
    Ok(drift)
}

/// Every file under `directory`, by its path from it joined with `/`; none where it does not
/// exist.
fn files_under(directory: &Path) -> io::Result<BTreeMap<String, Vec<u8>>> {
    let mut files = BTreeMap::new();
    if !directory.exists() {
        return Ok(files);
    }
    let mut pending = vec![directory.to_owned()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir)? {
            let path = entry?.path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            let relative: Vec<_> = path
                .strip_prefix(directory)
                .expect("a file lies under its directory")
                .components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect();
            files.insert(relative.join("/"), fs::read(&path)?);
        }
    }
    Ok(files)
}

/// Writes each package under `root` whole, and removes the directory of a package its host does
/// not get. Gives the directories written.
fn write_all(root: &Path, packages: &[Package]) -> io::Result<Vec<String>> {
    let mut written = Vec::new();
    for package in packages {
        let directory = root.join(&package.directory);
        match &package.files {
            Some(files) => {
                write::replace(&directory, files)?;
                written.push(package.directory.clone());
            }
            None if directory.exists() => fs::remove_dir_all(&directory)?,
            None => {}
        }
    }
    Ok(written)
}

/// Refuses a Codex discovery entry that is missing, copied or linked elsewhere than the
/// canonical skill source, naming the entry.
fn discovered(root: &Path) -> Result<(), String> {
    let entry = Host::Codex.directory();
    match discovery::preserved(root, Host::Codex) {
        Ok(true) => Ok(()),
        Ok(false) => Err(format!(
            "{entry}: the Codex discovery entry is missing or a copy; it must be the link to `{}`",
            discovery::LINK
        )),
        Err(error) => Err(format!("{entry}: {error}")),
    }
}

/// `loft skill package`: writes every package, or with `check` names each package file that
/// differs from its composition and exits 1 if any does.
pub fn run(root: &Path, check: bool, as_json: bool) -> Finished {
    if let Err(message) = discovered(root) {
        let error = json::error("discovery", &message, &[]);
        return json::stop(SCHEMA, vec![error], &message, as_json);
    }
    let packages = packages(&PLUGINS);
    let done = if check {
        drift(root, &packages).map(|drift| (Vec::new(), drift))
    } else {
        write_all(root, &packages).map(|written| (written, Drift::default()))
    };
    let (written, drift) = match done {
        Ok(done) => done,
        Err(error) => {
            let message = format!("plugins: cannot package the skills: {error}");
            let error = json::error("package", &message, &[]);
            return json::stop(SCHEMA, vec![error], &message, as_json);
        }
    };
    let outcome = match (check, drift.is_empty()) {
        (false, _) => "packaged",
        (true, true) => "current",
        (true, false) => "drift",
    };
    if as_json {
        let fields = json!({
            "packages": packages.iter().filter(|package| package.files.is_some()).map(|package| &package.directory).collect::<Vec<_>>(),
            "changed": drift.changed,
            "missing": drift.missing,
            "extra": drift.extra,
            "written": !check,
        });
        json::print(&json::result(SCHEMA, outcome, fields, Vec::new()));
    } else {
        report(&written, &drift, outcome);
    }
    if outcome == "drift" {
        Finished::failure(outcome)
    } else {
        Finished::success(outcome)
    }
}

/// Prints a line for each directory written, or for each file that differs.
fn report(written: &[String], drift: &Drift, outcome: &str) {
    for directory in written {
        outln!("packaged {directory}");
    }
    for (change, paths) in [
        ("changed", &drift.changed),
        ("missing", &drift.missing),
        ("extra", &drift.extra),
    ] {
        for path in paths {
            outln!("{change} {path}");
        }
    }
    match outcome {
        "current" => outln!("every package equals its composition"),
        "drift" => outln!("run `loft skill package` to rewrite the packages"),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use std::{env, fs, process};

    use super::{Drift, Host, Plugin, drift, packages, write_all};

    #[test]
    fn a_host_without_a_yard_adapter_gets_no_yard_package() {
        let plugins = [Plugin {
            host: Host::Codex,
            root: "plugins/codex/loft",
            yard: None,
        }];
        let packages = packages(&plugins);
        let directories: Vec<(&str, bool)> = packages
            .iter()
            .map(|package| (package.directory.as_str(), package.files.is_some()))
            .collect();
        assert_eq!(
            directories,
            [
                ("plugins/codex/loft/skills/loft-spec", true),
                ("plugins/codex/loft/skills/yard", false),
            ]
        );
        let root = env::temp_dir().join(format!("loft-cli-package-{}", process::id()));
        let yard = root.join("plugins/codex/loft/skills/yard");
        fs::create_dir_all(&yard).unwrap();
        fs::write(yard.join("SKILL.md"), "left from an adapter").unwrap();
        assert_eq!(
            drift(&root, &packages).unwrap().extra,
            ["plugins/codex/loft/skills/yard/SKILL.md"]
        );
        write_all(&root, &packages).unwrap();
        assert!(!yard.exists());
        assert_eq!(drift(&root, &packages).unwrap(), Drift::default());
        fs::remove_dir_all(root).unwrap();
    }
}
