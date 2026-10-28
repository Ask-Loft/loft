//! How one page server learns of another: `~/.yard/server.json` names the running one, which
//! holds the lock on `~/.yard/server.lock` while it runs; and a newer one, or one run to replace
//! it, asks it to stop through `~/.yard/stop.json`, which names the asking `loft`'s version and
//! whether it replaces the running one whatever its version. No connection is opened to learn
//! either.

use std::cmp::Ordering;

use serde_json::{Value, json};
use yardlog_store::home::{Home, HomeFile};

use crate::version::{VERSION, Version};
use crate::yard::failure::Failure;

pub use crate::yard::page::{Running, running};

/// How `version` compares with this `loft`'s: a version that does not read counts as older.
pub fn against_ours(version: &str) -> Ordering {
    against(version, VERSION)
}

/// How the version `theirs` compares with `ours`: a version that does not read is older than
/// every version that does.
fn against(theirs: &str, ours: &str) -> Ordering {
    Version::parse(theirs).cmp(&Version::parse(ours))
}

/// Asks the running page server to stop, naming this `loft`'s version, and whether it is to
/// stop whatever its own version, to be replaced.
pub fn request_stop(home: &Home, replace: bool) -> Result<(), Failure> {
    let mut request = json!({ "version": VERSION, "pid": std::process::id() });
    if replace {
        request["replace"] = json!(true);
    }
    home.write(HomeFile::Stop, request.to_string().as_bytes())?;
    Ok(())
}

/// Whether `~/.yard/stop.json` holds a request that replaces this page server, or one made by a
/// `loft` newer than this one.
pub fn asked_to_stop(home: &Home) -> bool {
    let Ok(Some(bytes)) = home.read(HomeFile::Stop) else {
        return false;
    };
    let Ok(request) = serde_json::from_slice::<Value>(&bytes) else {
        return false;
    };
    request["replace"] == true
        || request["version"].as_str().map(against_ours) == Some(Ordering::Greater)
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use super::against;

    #[test]
    fn a_prerelease_page_server_gives_way_to_its_release_and_replaces_an_earlier_one() {
        assert_eq!(against("2.3.0", "2.3.0-alpha.1"), Ordering::Greater);
        assert_eq!(against("2.3.0-alpha.1", "2.3.0"), Ordering::Less);
        assert_eq!(against("2.2.9", "2.3.0-alpha.1"), Ordering::Less);
        assert_eq!(against("2.3.0-alpha.1", "2.2.9"), Ordering::Greater);
        assert_eq!(against("2.3.0-alpha.1", "2.3.0-alpha.1"), Ordering::Equal);
    }

    #[test]
    fn a_page_server_of_an_unrecognised_version_is_older() {
        assert_eq!(against("2.3.0-pre", "0.0.1-alpha.0"), Ordering::Less);
    }
}
