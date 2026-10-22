//! Registering `loft merge-driver` in the repository that holds the tree (RQ-152, RQ-153): the
//! attribute that routes `spec/` files to the driver, beside `spec/`, and the driver's command
//! and name in the repository's local configuration, each written only when it differs. The
//! command that starts `loft` is the project's to give, such as `npx --no-install loft` for a
//! dev dependency; the install first checks that git can run it.

use std::fs;
use std::path::{Path, PathBuf};

use crate::StoreError;
use crate::fs::write_outputs;
use crate::repository::GitCommand;

/// The attributes file beside `spec/`; its patterns are relative to its own directory.
pub const ATTRIBUTES: &str = ".gitattributes";
/// The line that routes every file of the tree to the driver.
pub const ATTRIBUTE: &str = "spec/**/*.yaml merge=loft";
/// The configuration name that holds the driver's command.
pub const DRIVER: &str = "merge.loft.driver";
/// The command that starts `loft` unless the install is given another.
pub const LAUNCHER: &str = "loft";
/// The driver's description.
pub const NAME: (&str, &str) = ("merge.loft.name", "Loft Specification Format merge driver");

/// The shell command git runs as the driver: `<launcher> merge-driver %O %A %B %P` when
/// `<launcher> --version` runs; otherwise a text merge with the conflict markers of
/// `git merge-file`, which exits 1 even without a conflict, so the file stays unmerged and never
/// keeps one side silently.
#[must_use]
pub fn driver(launcher: &str) -> String {
    format!(
        "if {launcher} --version >/dev/null 2>&1; then {launcher} merge-driver %O %A %B %P; \
         else echo \"loft merge-driver could not run, so %P was merged as text with conflict \
         markers; make it run where git runs it, then run git checkout -m %P\" >&2; \
         git merge-file -L ours -L base -L theirs %A %O %B; exit 1; fi"
    )
}

/// What an install did: the names it wrote, the attributes file then each configuration name it
/// set, and the version the launcher printed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed {
    pub written: Vec<String>,
    pub version: String,
}

/// Registers the driver for the tree under `root`, started by `launcher`. A second run with the
/// same launcher writes nothing.
///
/// # Errors
///
/// `root` is not inside a git repository, or git cannot run `<launcher> --version`, in either
/// case before anything is written; git fails; or the attributes file cannot be read or written.
pub fn install(git: &GitCommand, root: &Path, launcher: &str) -> Result<Installed, StoreError> {
    install_routing(git, root, launcher, &[ATTRIBUTE])
}

/// Registers the driver as [`install`] does, routing to it the files each of `attributes`
/// names, each line added in its order where the attributes file does not hold it.
///
/// # Errors
///
/// As [`install`].
pub fn install_routing(
    git: &GitCommand,
    root: &Path,
    launcher: &str,
    attributes: &[&str],
) -> Result<Installed, StoreError> {
    git.prefix(root)?;
    let version = git
        .launcher_version(root, launcher)?
        .ok_or_else(|| StoreError::Launcher(launcher.to_owned()))?;
    let mut written = Vec::new();
    let path = root.join(ATTRIBUTES);
    let current = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(crate::error::io_error(&path)(error)),
    };
    let missing: Vec<&str> = attributes
        .iter()
        .copied()
        .filter(|attribute| !current.lines().any(|line| line.trim() == *attribute))
        .collect();
    if !missing.is_empty() {
        let separator = if current.is_empty() || current.ends_with('\n') {
            ""
        } else {
            "\n"
        };
        let text = missing
            .iter()
            .fold(format!("{current}{separator}"), |mut text, line| {
                text.push_str(line);
                text.push('\n');
                text
            });
        write_outputs(root, &[(PathBuf::from(ATTRIBUTES), text.into_bytes())])?;
        written.push(ATTRIBUTES.to_owned());
    }
    for (name, value) in [(DRIVER, driver(launcher).as_str()), NAME] {
        if git.config_get(root, name)?.as_deref() != Some(value) {
            git.config_set(root, name, value)?;
            written.push(name.to_owned());
        }
    }
    Ok(Installed { written, version })
}
