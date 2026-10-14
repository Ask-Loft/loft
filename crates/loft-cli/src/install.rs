//! How the running `loft` was installed: the way the install record states for its own
//! executable, or, where no record names it, the way inferred from where the executable lies.
//! Nothing here prints or fails: a record that is missing, unreadable or damaged reads as none.

use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde_json::{Value, json};

use crate::settings::{self, SettingsError};
use crate::version::VERSION;

/// The settings file that holds the install records.
const FILE: &str = "install.json";

/// A way of installing `loft`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Way {
    Npm,
    Cargo,
    ReleaseBinary,
    Unknown,
}

impl Way {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Npm => "npm",
            Self::Cargo => "cargo",
            Self::ReleaseBinary => "release-binary",
            Self::Unknown => "unknown",
        }
    }

    fn parse(name: &str) -> Option<Self> {
        match name {
            "npm" => Some(Self::Npm),
            "cargo" => Some(Self::Cargo),
            "release-binary" => Some(Self::ReleaseBinary),
            _ => None,
        }
    }
}

/// The running executable's own path, with every link resolved.
pub fn executable() -> Option<PathBuf> {
    env::current_exe()
        .ok()
        .and_then(|path| fs::canonicalize(path).ok())
}

/// Records `way` for the running executable, replacing an earlier record for its path and
/// keeping those of other paths. Gives the path recorded.
///
/// # Errors
///
/// The record cannot be written.
pub fn record(way: Way) -> Result<PathBuf, SettingsError> {
    let path = executable().unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_NAME")));
    let mut records = settings::read(FILE);
    let entry = json!({ "path": path, "way": way.as_str(), "version": VERSION });
    let mut installs: Vec<Value> = records["installs"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|install| install["path"].as_str() != path.to_str())
        .collect();
    installs.push(entry);
    records["installs"] = Value::Array(installs);
    settings::write(FILE, &records)?;
    Ok(path)
}

/// How the running `loft` was installed.
#[expect(dead_code, reason = "the command event and the update notice read it")]
pub fn way() -> Way {
    let Some(executable) = executable() else {
        return Way::Unknown;
    };
    recorded(&settings::read(FILE), &executable)
        .unwrap_or_else(|| inferred(&executable, cargo_home().as_deref()))
}

/// The way `records` state for `executable`, if one names its path.
fn recorded(records: &Value, executable: &Path) -> Option<Way> {
    records["installs"]
        .as_array()?
        .iter()
        .rev()
        .find(|install| install["path"].as_str() == executable.to_str())
        .and_then(|install| Way::parse(install["way"].as_str()?))
}

/// The way inferred from where `executable` lies: inside `node_modules/@ask-loft/` it was
/// installed by npm; in cargo's binary directory, which cargo's list of installed crates names
/// under `loft-cli`, by cargo; anywhere else the way is unknown.
fn inferred(executable: &Path, cargo_home: Option<&Path>) -> Way {
    let parts: Vec<Component<'_>> = executable.components().collect();
    let in_npm_scope = parts
        .windows(2)
        .any(|pair| pair[0].as_os_str() == "node_modules" && pair[1].as_os_str() == "@ask-loft");
    if in_npm_scope {
        return Way::Npm;
    }
    match cargo_home {
        Some(home) if installed_by_cargo(executable, home) => Way::Cargo,
        _ => Way::Unknown,
    }
}

/// Whether `executable` lies in `home/bin` and `home/.crates2.json` lists `loft-cli` with it.
fn installed_by_cargo(executable: &Path, home: &Path) -> bool {
    let bin = fs::canonicalize(home.join("bin")).unwrap_or_else(|_| home.join("bin"));
    if executable.parent() != Some(bin.as_path()) {
        return false;
    }
    let Some(name) = executable.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let listed = fs::read(home.join(".crates2.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
    listed.is_some_and(|listed| {
        listed["installs"].as_object().is_some_and(|installs| {
            installs.iter().any(|(package, install)| {
                package.starts_with("loft-cli ")
                    && install["bins"]
                        .as_array()
                        .is_some_and(|bins| bins.iter().any(|bin| bin.as_str() == Some(name)))
            })
        })
    })
}

/// Cargo's home: `CARGO_HOME`, or `.cargo` in the home directory.
fn cargo_home() -> Option<PathBuf> {
    let variable = |name: &str| env::var_os(name).filter(|value| !value.is_empty());
    variable("CARGO_HOME").map(PathBuf::from).or_else(|| {
        variable(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
            .map(|home| PathBuf::from(home).join(".cargo"))
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use serde_json::json;

    use super::{Way, inferred, recorded};

    #[test]
    fn a_record_for_the_executable_gives_its_way_and_one_for_another_path_does_not() {
        let records = json!({ "installs": [
            { "path": "/elsewhere/loft", "way": "cargo", "version": "2.0.0" },
            { "path": "/here/loft", "way": "npm", "version": "2.0.0" },
        ] });
        assert_eq!(recorded(&records, Path::new("/here/loft")), Some(Way::Npm));
        assert_eq!(recorded(&records, Path::new("/other/loft")), None);
    }

    #[test]
    fn a_damaged_record_gives_no_way() {
        for records in [
            json!({}),
            json!({ "installs": "npm" }),
            json!({ "installs": [{ "path": "/here/loft", "way": "brew" }] }),
            json!({ "installs": [{ "path": 1, "way": "npm" }] }),
        ] {
            assert_eq!(
                recorded(&records, Path::new("/here/loft")),
                None,
                "{records}"
            );
        }
    }

    #[test]
    fn an_executable_inside_node_modules_at_ask_loft_was_installed_by_npm() {
        let path = Path::new("/p/node_modules/@ask-loft/loft-linux-x64/bin/loft");
        assert_eq!(inferred(path, None), Way::Npm);
        let unscoped = Path::new("/p/node_modules/loft/bin/loft");
        assert_eq!(inferred(unscoped, None), Way::Unknown);
    }

    #[test]
    fn an_executable_in_cargo_s_bin_listed_under_loft_cli_was_installed_by_cargo() {
        let home = std::env::temp_dir().join(format!("loft-cargo-home-{}", std::process::id()));
        fs::create_dir_all(home.join("bin")).unwrap();
        let bin = fs::canonicalize(home.join("bin")).unwrap();
        let executable = bin.join("loft");
        assert_eq!(inferred(&executable, Some(&home)), Way::Unknown);
        let listed = json!({ "installs": {
            "loft-cli 2.0.3 (registry+https://github.com/rust-lang/crates.io-index)":
                { "bins": ["loft"] },
        } });
        fs::write(home.join(".crates2.json"), listed.to_string()).unwrap();
        assert_eq!(inferred(&executable, Some(&home)), Way::Cargo);
        let elsewhere = Path::new("/usr/local/bin/loft");
        assert_eq!(inferred(elsewhere, Some(&home)), Way::Unknown);
        fs::remove_dir_all(&home).unwrap();
    }
}
