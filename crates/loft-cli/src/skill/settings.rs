//! Enabling the Claude Code plugin `loft@ask-loft` beside the skill (Writing the skill): the
//! marketplace `ask-loft`, pinned to the release of the running `loft`, and the plugin go into
//! the project's `.claude/settings.json`, so that Claude Code offers the plugin's `yard` skill and
//! Yard Client of that version to everyone who trusts the project. Every other entry is kept in
//! the file's own order; a marketplace from this repository is moved to the running release, any
//! other entry the file already sets is kept as it is, and a file that is not a JSON object is
//! left alone.

use std::fmt;
use std::fs;
use std::io;
use std::path::Path;
use std::process;

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{Serialize, SerializeMap, SerializeSeq, Serializer};
use serde_json::Number;

use crate::version::VERSION;

/// The file, from the project's directory.
pub const SETTINGS: &str = ".claude/settings.json";
/// The marketplace and the plugin the file enables.
pub const MARKETPLACE: &str = "ask-loft";
pub const PLUGIN: &str = "loft@ask-loft";
const REPOSITORY: &str = "Ask-Loft/loft";

/// The release tag the marketplace is pinned to, so that the plugin is of the running version.
fn release() -> String {
    format!("v{VERSION}")
}

/// What enabling the plugin did to the file.
#[derive(Debug, PartialEq, Eq)]
pub enum Enabled {
    /// The file was created or given an entry it lacked.
    Written,
    /// The file already held both entries and was not written.
    Unchanged,
    /// The file is not a JSON object, so it was left as it is.
    NotAnObject,
}

/// Adds the marketplace and the plugin to `.claude/settings.json` under `root` where the file
/// lacks them.
///
/// # Errors
///
/// The file cannot be read or written.
pub fn enable(root: &Path) -> io::Result<Enabled> {
    let path = root.join(SETTINGS);
    let mut settings = match fs::read_to_string(&path) {
        Ok(text) => match serde_json::from_str::<Ordered>(&text) {
            Ok(Ordered::Object(entries)) => entries,
            _ => return Ok(Enabled::NotAnObject),
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error),
    };
    let marketplace = Ordered::Object(vec![(
        "source".to_owned(),
        Ordered::Object(vec![
            ("source".to_owned(), Ordered::String("github".to_owned())),
            ("repo".to_owned(), Ordered::String(REPOSITORY.to_owned())),
            ("ref".to_owned(), Ordered::String(release())),
        ]),
    )]);
    let added = [
        pin(&mut settings, marketplace),
        add(&mut settings, "enabledPlugins", PLUGIN, Ordered::Bool(true)),
    ];
    match added {
        [Added::No, Added::No] => Ok(Enabled::Unchanged),
        [Added::NotAnObject, _] | [_, Added::NotAnObject] => Ok(Enabled::NotAnObject),
        _ => {
            let mut text = serde_json::to_string_pretty(&Ordered::Object(settings))
                .expect("settings serialise");
            text.push('\n');
            write(&path, &text)?;
            Ok(Enabled::Written)
        }
    }
}

/// What a hand edit of the file needs, for a file the command leaves alone.
pub fn by_hand() -> String {
    format!(
        "{SETTINGS} does not hold a JSON object these entries fit in, so it was left as it is; \
         to enable the Loft plugin, add \"extraKnownMarketplaces\": {{\"{MARKETPLACE}\": {{\"source\": {{\"source\": \
         \"github\", \"repo\": \"{REPOSITORY}\", \"ref\": \"{}\"}}}}}} and \"enabledPlugins\": {{\"{PLUGIN}\": true}}",
        release()
    )
}

enum Added {
    Yes,
    No,
    NotAnObject,
}

/// Puts the marketplace into `extraKnownMarketplaces` where it is missing, and moves a marketplace
/// `ask-loft` from this repository to the running release, keeping its other fields; one from any
/// other source is kept as it is.
fn pin(settings: &mut Vec<(String, Ordered)>, marketplace: Ordered) -> Added {
    let added = add(settings, "extraKnownMarketplaces", MARKETPLACE, marketplace);
    if !matches!(added, Added::No) {
        return added;
    }
    let Some(source) = ours(settings) else {
        return Added::No;
    };
    let release = Ordered::String(release());
    match source.iter_mut().find(|(name, _)| name == "ref") {
        Some((_, current)) if *current == release => Added::No,
        Some((_, current)) => {
            *current = release;
            Added::Yes
        }
        None => {
            source.push(("ref".to_owned(), release));
            Added::Yes
        }
    }
}

/// The `source` of the marketplace `ask-loft` where it is this repository on GitHub.
fn ours(settings: &mut [(String, Ordered)]) -> Option<&mut Vec<(String, Ordered)>> {
    let (_, Ordered::Object(marketplaces)) = settings
        .iter_mut()
        .find(|(name, _)| name == "extraKnownMarketplaces")?
    else {
        return None;
    };
    let (_, Ordered::Object(entry)) = marketplaces
        .iter_mut()
        .find(|(name, _)| name == MARKETPLACE)?
    else {
        return None;
    };
    let (_, Ordered::Object(source)) = entry.iter_mut().find(|(name, _)| name == "source")? else {
        return None;
    };
    let field = |name: &str| {
        source
            .iter()
            .find(|(field, _)| field == name)
            .map(|(_, value)| value.clone())
    };
    let github = field("source") == Some(Ordered::String("github".to_owned()));
    let repository = field("repo") == Some(Ordered::String(REPOSITORY.to_owned()));
    (github && repository).then_some(source)
}

/// Puts `key: value` into the object `settings[section]`, creating it last where it is missing,
/// unless the object already holds `key`.
fn add(settings: &mut Vec<(String, Ordered)>, section: &str, key: &str, value: Ordered) -> Added {
    let at = settings
        .iter()
        .position(|(name, _)| name == section)
        .unwrap_or_else(|| {
            settings.push((section.to_owned(), Ordered::Object(Vec::new())));
            settings.len() - 1
        });
    let Ordered::Object(entries) = &mut settings[at].1 else {
        return Added::NotAnObject;
    };
    if entries.iter().any(|(name, _)| name == key) {
        return Added::No;
    }
    entries.push((key.to_owned(), value));
    Added::Yes
}

/// Writes `text` beside `path`, then renames it into place, so the file is never half written.
fn write(path: &Path, text: &str) -> io::Result<()> {
    let dir = path.parent().expect("the settings file lies in `.claude/`");
    fs::create_dir_all(dir)?;
    let staged = dir.join(format!(".settings.json.new-{}", process::id()));
    fs::write(&staged, text)?;
    fs::rename(&staged, path).inspect_err(|_| {
        fs::remove_file(&staged).ok();
    })
}

/// A JSON value whose objects keep their entries in the order the text gives them.
#[derive(Debug, Clone, PartialEq)]
enum Ordered {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<Ordered>),
    Object(Vec<(String, Ordered)>),
}

impl<'de> Deserialize<'de> for Ordered {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(OrderedVisitor)
    }
}

struct OrderedVisitor;

impl<'de> Visitor<'de> for OrderedVisitor {
    type Value = Ordered;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_unit<E>(self) -> Result<Ordered, E> {
        Ok(Ordered::Null)
    }

    fn visit_bool<E>(self, value: bool) -> Result<Ordered, E> {
        Ok(Ordered::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Ordered, E> {
        Ok(Ordered::Number(value.into()))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Ordered, E> {
        Ok(Ordered::Number(value.into()))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Ordered, E> {
        Number::from_f64(value)
            .map(Ordered::Number)
            .ok_or_else(|| E::custom("a number JSON cannot hold"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Ordered, E> {
        Ok(Ordered::String(value.to_owned()))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Ordered, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        Ok(Ordered::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Ordered, A::Error> {
        let mut entries: Vec<(String, Ordered)> = Vec::new();
        while let Some((key, value)) = map.next_entry::<String, Ordered>()? {
            // A key written twice takes its last value, in its first place, as JSON readers do.
            match entries.iter_mut().find(|(name, _)| *name == key) {
                Some(entry) => entry.1 = value,
                None => entries.push((key, value)),
            }
        }
        Ok(Ordered::Object(entries))
    }
}

impl Serialize for Ordered {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Null => serializer.serialize_unit(),
            Self::Bool(value) => serializer.serialize_bool(*value),
            Self::Number(value) => value.serialize(serializer),
            Self::String(value) => serializer.serialize_str(value),
            Self::Array(items) => {
                let mut seq = serializer.serialize_seq(Some(items.len()))?;
                for item in items {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
            Self::Object(entries) => {
                let mut map = serializer.serialize_map(Some(entries.len()))?;
                for (key, value) in entries {
                    map.serialize_entry(key, value)?;
                }
                map.end()
            }
        }
    }
}
