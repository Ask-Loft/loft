//! Registering `loft merge-driver` in the repository that holds the tree (RQ-152, RQ-153): the
//! attribute that routes `spec/` files to the driver, beside `spec/`, and the driver's command
//! and name in the repository's local configuration, each written only when it is missing.

use std::fs;
use std::path::{Path, PathBuf};

use crate::StoreError;
use crate::fs::write_outputs;
use crate::repository::GitCommand;

/// The attributes file beside `spec/`; its patterns are relative to its own directory.
pub const ATTRIBUTES: &str = ".gitattributes";
/// The line that routes every file of the tree to the driver.
pub const ATTRIBUTE: &str = "spec/**/*.yaml merge=loft";
/// The driver's command, with the placeholders git fills in.
pub const DRIVER: (&str, &str) = ("merge.loft.driver", "loft merge-driver %O %A %B %P");
/// The driver's description.
pub const NAME: (&str, &str) = ("merge.loft.name", "Loft Specification Format merge driver");

/// Registers the driver for the tree under `root`, and names what it wrote: the attributes file,
/// then each configuration name it set. A second run writes nothing.
///
/// # Errors
///
/// `root` is not inside a git repository, in which case nothing is written; git fails; or the
/// attributes file cannot be read or written.
pub fn install(git: &GitCommand, root: &Path) -> Result<Vec<String>, StoreError> {
    git.prefix(root)?;
    let mut written = Vec::new();
    let path = root.join(ATTRIBUTES);
    let current = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(crate::error::io_error(&path)(error)),
    };
    if !current.lines().any(|line| line.trim() == ATTRIBUTE) {
        let separator = if current.is_empty() || current.ends_with('\n') {
            ""
        } else {
            "\n"
        };
        let text = format!("{current}{separator}{ATTRIBUTE}\n");
        write_outputs(root, &[(PathBuf::from(ATTRIBUTES), text.into_bytes())])?;
        written.push(ATTRIBUTES.to_owned());
    }
    for (name, value) in [DRIVER, NAME] {
        if git.config_get(root, name)?.as_deref() != Some(value) {
            git.config_set(root, name, value)?;
            written.push(name.to_owned());
        }
    }
    Ok(written)
}
