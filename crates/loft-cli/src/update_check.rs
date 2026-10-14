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
use crate::version::VERSION;

/// How long an answer, and a notice, last.
const DAY: u64 = 86_400;

/// Prints the update notice when the kept answer names a release newer than this `loft` and
/// no notice was printed in the last day, and gives the work that asks GitHub again when the
/// kept answer is missing or more than a day old. Nothing while the update check is off.
pub fn after_run() -> Option<Job> {
    if !UPDATE_CHECK.state().on {
        return None;
    }
    let now = SystemClock::from_env().now().ok()?;
    let mut kept = settings::read(UPDATE_CHECK.file);
    if let Some(notice) = notice(&kept, now) {
        errln!("{notice}");
        kept["noticeAt"] = json!(now);
        let _ = settings::write(UPDATE_CHECK.file, &kept);
    }
    let fresh = kept["receivedAt"]
        .as_u64()
        .is_some_and(|received| now < received.saturating_add(DAY));
    if fresh {
        return None;
    }
    let address = endpoints::latest_release();
    Some(Box::new(move |left: Duration| {
        if let Some((200, body)) = network::get_json(&address, left) {
            keep(&body, now);
        }
    }))
}

/// The notice for the answer `kept` holds at `now`, if one is due.
fn notice(kept: &Value, now: u64) -> Option<String> {
    let recent = kept["noticeAt"]
        .as_u64()
        .is_some_and(|at| now < at.saturating_add(DAY));
    if recent {
        return None;
    }
    let latest = kept["latest"]["tag"].as_str()?.strip_prefix('v')?;
    let page = kept["latest"]["page"].as_str()?;
    if parse(latest)? <= parse(VERSION)? {
        return None;
    }
    let how = match install::way() {
        Way::Npm => "update with `npm install @ask-loft/loft@latest`".to_owned(),
        Way::Cargo => "update with `cargo install --locked loft-cli`".to_owned(),
        Way::ReleaseBinary | Way::Unknown => format!("download it from {page}"),
    };
    Some(format!(
        "loft: loft {latest} is available; this is {VERSION}. To update, {how}."
    ))
}

/// Keeps the release that `body`, GitHub's answer, names as the latest, received at `now`.
/// A draft or a prerelease is not taken as the latest; an answer that does not read leaves
/// what was kept as it was.
fn keep(body: &[u8], now: u64) {
    let Ok(release) = serde_json::from_slice::<Value>(body) else {
        return;
    };
    let Some(tag) = release["tag_name"].as_str() else {
        return;
    };
    let mut kept = settings::read(UPDATE_CHECK.file);
    let published = release["draft"] == json!(false) && release["prerelease"] == json!(false);
    if published {
        let version = tag.strip_prefix('v').unwrap_or(tag);
        let page = release["html_url"].as_str().unwrap_or_default();
        kept["latest"] = json!({ "version": version, "tag": tag, "page": page });
    }
    kept["receivedAt"] = json!(now);
    let _ = settings::write(UPDATE_CHECK.file, &kept);
}

/// The numbers of a version `X.Y.Z`, if it is one.
fn parse(version: &str) -> Option<(u64, u64, u64)> {
    let mut parts = version.split('.');
    let mut number = || {
        parts
            .next()
            .filter(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
            .and_then(|part| part.parse().ok())
    };
    let parsed = (number()?, number()?, number()?);
    parts.next().is_none().then_some(parsed)
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn only_three_numbers_parse_as_a_version() {
        assert_eq!(parse("2.10.3"), Some((2, 10, 3)));
        for text in [
            "2.1",
            "2.1.0.1",
            "2.1.0-rc.1",
            "nightly",
            "",
            "v2.1.0",
            "2..1",
        ] {
            assert_eq!(parse(text), None, "{text}");
        }
    }

    #[test]
    fn versions_compare_by_their_numbers() {
        assert!(parse("2.10.0") > parse("2.9.9"));
        assert!(parse("3.0.0") > parse("2.99.99"));
    }
}
