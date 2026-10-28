//! `loft --version` (F-46): the tool release, the format and the hashing algorithm, on three
//! lines (AC-589..AC-591) or as one JSON document (AC-682).

use loftspec::FORMAT;
use loftspec::manifest::ALGORITHM;
use serde_json::json;

use crate::json;
use crate::out::outln;

/// The schema `--format json` writes to, by name and version (RQ-156).
const SCHEMA: &str = "loft-version/1";

/// The tool release, from the crate that builds `loft`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn run(as_json: bool) {
    if as_json {
        json::print(&json!({
            "schema": SCHEMA,
            "version": VERSION,
            "format": FORMAT,
            "algorithm": ALGORITHM,
        }));
    } else {
        outln!("loft {VERSION}");
        outln!("format {FORMAT}");
        outln!("algorithm {ALGORITHM}");
    }
}

/// A version of `loft`: a release `X.Y.Z`, or a prerelease of it, `X.Y.Z-alpha.N`,
/// `X.Y.Z-beta.N` or `X.Y.Z-rc.N`. Versions order by their numbers, and a prerelease comes
/// before the release of its numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    numbers: (u64, u64, u64),
    stage: Stage,
}

/// How far a version has come towards the release of its numbers, in that order: `alpha`
/// before `beta` before `rc` before the release, and prereleases of one identifier by number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Stage {
    Alpha(u64),
    Beta(u64),
    Rc(u64),
    Release,
}

impl Version {
    /// The version `text` names, if it is a release or a prerelease of the accepted forms. An
    /// unrecognised version is `None`, which orders before every `Some`.
    pub fn parse(text: &str) -> Option<Self> {
        let (release, prerelease) = match text.split_once('-') {
            Some((release, prerelease)) => (release, Some(prerelease)),
            None => (text, None),
        };
        let mut parts = release.split('.');
        let mut next = || parts.next().and_then(number);
        let numbers = (next()?, next()?, next()?);
        if parts.next().is_some() {
            return None;
        }
        let stage = match prerelease.map(|text| text.split_once('.')) {
            None => Stage::Release,
            Some(Some(("alpha", n))) => Stage::Alpha(number(n)?),
            Some(Some(("beta", n))) => Stage::Beta(number(n)?),
            Some(Some(("rc", n))) => Stage::Rc(number(n)?),
            Some(_) => return None,
        };
        Some(Self { numbers, stage })
    }
}

/// The number `part` spells in decimal digits alone, if it does.
fn number(part: &str) -> Option<u64> {
    if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    part.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::Version;

    fn parse(text: &str) -> Version {
        Version::parse(text).unwrap_or_else(|| panic!("{text} is a version"))
    }

    #[test]
    fn releases_and_the_three_prerelease_forms_parse_as_versions() {
        for text in ["2.10.3", "2.3.0-alpha.1", "2.3.0-beta.12", "2.3.0-rc.0"] {
            assert!(Version::parse(text).is_some(), "{text}");
        }
        for text in [
            "2.1",
            "2.1.0.1",
            "2.1.0-pre",
            "2.1.0-alpha",
            "2.1.0-alpha.",
            "2.1.0-alpha.1.2",
            "2.1.0-dev.1",
            "2.1.0-",
            "nightly",
            "",
            "v2.1.0",
            "2..1",
        ] {
            assert_eq!(Version::parse(text), None, "{text}");
        }
    }

    #[test]
    fn versions_compare_by_their_numbers() {
        assert!(parse("2.10.0") > parse("2.9.9"));
        assert!(parse("3.0.0") > parse("2.99.99"));
        assert!(parse("2.3.0-alpha.1") > parse("2.2.9"));
    }

    #[test]
    fn a_prerelease_comes_before_its_release_and_alpha_before_beta_before_rc() {
        let order = [
            "2.3.0-alpha.1",
            "2.3.0-alpha.2",
            "2.3.0-alpha.10",
            "2.3.0-beta.1",
            "2.3.0-rc.1",
            "2.3.0-rc.2",
            "2.3.0",
            "2.3.1-alpha.1",
        ];
        for pair in order.windows(2) {
            assert!(parse(pair[0]) < parse(pair[1]), "{} < {}", pair[0], pair[1]);
        }
    }

    #[test]
    fn two_of_the_same_prerelease_are_equal() {
        assert_eq!(parse("2.3.0-beta.4"), parse("2.3.0-beta.4"));
        assert_eq!(
            Version::parse("2.3.0-beta.4").cmp(&Version::parse("2.3.0-beta.4")),
            std::cmp::Ordering::Equal
        );
    }

    #[test]
    fn an_unrecognised_version_is_earlier_than_every_recognised_one() {
        for unrecognised in ["2.3.0-pre", "nightly", "2.3.0.1"] {
            for recognised in ["0.0.0-alpha.0", "0.0.0", "2.3.0-rc.1"] {
                assert!(
                    Version::parse(unrecognised) < Version::parse(recognised),
                    "{unrecognised} < {recognised}"
                );
            }
        }
    }
}
