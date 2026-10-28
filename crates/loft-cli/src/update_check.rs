//! The update check: at the end of a run, at most once a day, `loft` asks GitHub for the latest
//! release and keeps the answer; a later run tells of a newer release, at most once a day, with
//! the way to install it that matches how this `loft` was installed. No run waits for GitHub
//! before its work, the request carries nothing but the address, and nothing about a failed
//! request reaches a person.

use std::time::Duration;

use loftspec_store::clock::{Clock, SystemClock};
use serde_json::{Value, json};

use crate::endpoints;
use crate::install::{self, Way};
use crate::network::{self, Job};
use crate::out::errln;
use crate::settings;
use crate::switches::UPDATE_CHECK;
use crate::version::{VERSION, Version};

/// How long an answer, and a notice, last.
const DAY: u64 = 86_400;

/// Prints the update notice when the kept answer names a release newer than this `loft` and
/// no notice was printed in the last day, and gives the work that asks GitHub again when the
/// kept answer is missing or more than a day old. Nothing while the update check is off, and
/// nothing when the settings cannot be written, since an answer or a notice that cannot be
/// kept would be asked for and printed again on every run.
pub fn after_run() -> Option<Job> {
    if !UPDATE_CHECK.state().on {
        return None;
    }
    let now = SystemClock::from_env().now().ok()?;
    let mut kept = settings::read(UPDATE_CHECK.file);
    let due = notice(&kept, now);
    let stale = kept["receivedAt"]
        .as_u64()
        .is_none_or(|received| now >= received.saturating_add(DAY));
    if due.is_none() && !stale {
        return None;
    }
    if due.is_some() {
        kept["noticeAt"] = json!(now);
    }
    settings::write(UPDATE_CHECK.file, &kept).ok()?;
    if let Some(notice) = due {
        errln!("{notice}");
    }
    stale.then(|| -> Job {
        let address = endpoints::latest_release();
        Box::new(move |left: Duration| {
            if let Some((200, body)) = network::get_json(&address, left) {
                keep(&body, now, kept);
            }
        })
    })
}

/// The notice for the answer `kept` holds at `now`, if one is due.
fn notice(kept: &Value, now: u64) -> Option<String> {
    notice_for(kept, now, VERSION)
}

/// The notice for the answer `kept` holds at `now` to a `loft` of the version `running`, if one
/// is due: a release newer than `running`, and no notice in the last day.
fn notice_for(kept: &Value, now: u64, running: &str) -> Option<String> {
    let recent = kept["noticeAt"]
        .as_u64()
        .is_some_and(|at| now < at.saturating_add(DAY));
    if recent {
        return None;
    }
    let latest = kept["latest"]["tag"].as_str()?.strip_prefix('v')?;
    let page = kept["latest"]["page"].as_str()?;
    if Version::parse(latest) <= Version::parse(running) {
        return None;
    }
    let how = match install::way() {
        Way::Npm => "update with `npm install @ask-loft/loft@latest`".to_owned(),
        Way::Cargo => "update with `cargo install --locked loft-cli`".to_owned(),
        Way::ReleaseBinary | Way::Unknown => format!("download it from {page}"),
    };
    Some(format!(
        "loft: loft {latest} is available; this is {running}. To update, {how}."
    ))
}

/// Keeps in `kept`, the settings as this run read them, the release that `body`, GitHub's
/// answer, names as the latest, received at `now`. A draft or a prerelease is not taken as the
/// latest; an answer that does not read leaves what was kept as it was.
fn keep(body: &[u8], now: u64, mut kept: Value) {
    let Ok(release) = serde_json::from_slice::<Value>(body) else {
        return;
    };
    let Some(tag) = release["tag_name"].as_str() else {
        return;
    };
    let published = release["draft"] == json!(false) && release["prerelease"] == json!(false);
    if published {
        let version = tag.strip_prefix('v').unwrap_or(tag);
        let page = release["html_url"].as_str().unwrap_or_default();
        kept["latest"] = json!({ "version": version, "tag": tag, "page": page });
    }
    kept["receivedAt"] = json!(now);
    let _ = settings::write(UPDATE_CHECK.file, &kept);
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::notice_for;

    fn kept(latest: &str) -> serde_json::Value {
        json!({ "latest": { "tag": format!("v{latest}"), "page": "https://example.test/release" } })
    }

    #[test]
    fn a_prerelease_is_told_of_the_release_of_its_numbers() {
        let notice = notice_for(&kept("2.3.0"), 0, "2.3.0-alpha.1");
        assert!(
            notice.as_deref().is_some_and(
                |notice| notice.contains("loft 2.3.0 is available; this is 2.3.0-alpha.1")
            ),
            "{notice:?}"
        );
    }

    #[test]
    fn a_release_is_not_told_of_itself_or_an_earlier_one() {
        assert_eq!(notice_for(&kept("2.3.0"), 0, "2.3.0"), None);
        assert_eq!(notice_for(&kept("2.2.9"), 0, "2.3.0-alpha.1"), None);
    }
}
