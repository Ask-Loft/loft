//! The repository's live Codex source is discovered through one known relative link.
//! Inspect destinations before staging any package, and never write through a link.

use std::fs;
use std::io;
use std::path::Path;

use super::compose::Host;

const SOURCE: &str = "crates/loft-cli/skill/loft-spec";
pub const LINK: &str = "../../crates/loft-cli/skill/loft-spec";

/// Whether this destination is the valid repository source link, preserved as it stands.
pub fn preserved(root: &Path, host: Host) -> io::Result<bool> {
    let directory = root.join(host.directory());
    let metadata = match fs::symlink_metadata(&directory) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if metadata.is_dir() {
        return Ok(false);
    }
    if metadata.file_type().is_symlink() {
        let source = root.join(SOURCE);
        if host == Host::Codex
            && fs::read_link(&directory)? == Path::new(LINK)
            && source.join("SKILL.md").is_file()
            && source.join("agents/openai.yaml").is_file()
            && directory.canonicalize()? == source.canonicalize()?
        {
            return Ok(true);
        }
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "refusing a broken or unexpected symbolic link; only the repository's live Codex source link can be preserved",
        ));
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "refusing to replace a non-directory skill destination",
    ))
}
