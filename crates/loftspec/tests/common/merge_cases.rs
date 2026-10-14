//! The cases of the merge properties: a file of the full document, a base made from it by
//! random edits, and two sides made from the base by independent random edits.

use std::fmt;
use std::sync::LazyLock;

use loftspec::codec::schema::Kind;
use loftspec::codec::{self, Files, SpecPath};
use loftspec::merge::{MergeError, Merged};
use proptest::prelude::*;

use super::edit_tree::Top;
use super::edits::{Edit, Side, edit, edited, growth};
use super::full::full_nested_document;
use super::merge_keys::Versions;
use super::text::{COMPONENT, FEATURE, PRODUCT};

/// A file the cases edit, and what it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fixture {
    pub path: &'static str,
    pub top: Top,
}

pub const FIXTURES: [Fixture; 4] = [
    Fixture {
        path: FEATURE,
        top: Top::Record(Kind::Feature),
    },
    Fixture {
        path: COMPONENT,
        top: Top::Record(Kind::RootComponent),
    },
    Fixture {
        path: PRODUCT,
        top: Top::Record(Kind::Product),
    },
    Fixture {
        path: codec::TAGS,
        top: Top::Registry,
    },
];

/// The full document's files, with its nested Component filled.
pub fn fixture_files() -> &'static Files {
    static FILES: LazyLock<Files> = LazyLock::new(|| codec::write(&full_nested_document()));
    &FILES
}

/// Three versions of one file and the edits that made them.
#[derive(Clone)]
pub struct Case {
    pub fixture: Fixture,
    pub base: Vec<u8>,
    pub ours: Vec<u8>,
    pub theirs: Vec<u8>,
    pub edits: [Vec<Edit>; 3],
}

impl fmt::Debug for Case {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
        writeln!(formatter, "file {}", self.fixture.path)?;
        writeln!(formatter, "edits {:?}", self.edits)?;
        writeln!(formatter, "--- base\n{}", text(&self.base))?;
        writeln!(formatter, "--- ours\n{}", text(&self.ours))?;
        write!(formatter, "--- theirs\n{}", text(&self.theirs))
    }
}

/// A file merge: `merge_file`, or `merge_file_by_key`, which is to replace it.
pub type Engine = fn(&SpecPath, &[u8], &[u8], &[u8]) -> Result<Merged, MergeError>;

impl Case {
    pub fn merge(
        &self,
        engine: Engine,
        base: &[u8],
        ours: &[u8],
        theirs: &[u8],
    ) -> Result<Merged, MergeError> {
        engine(&SpecPath::new(self.fixture.path), base, ours, theirs)
    }

    /// The merge of the case's three versions.
    pub fn merged(&self, engine: Engine) -> Result<Merged, MergeError> {
        self.merge(engine, &self.base, &self.ours, &self.theirs)
    }

    /// The merge with the sides swapped.
    pub fn swapped(&self, engine: Engine) -> Result<Merged, MergeError> {
        self.merge(engine, &self.base, &self.theirs, &self.ours)
    }

    /// The case's three versions, by key.
    pub fn versions(&self) -> Versions {
        Versions::of(self.fixture.top, [&self.base, &self.ours, &self.theirs])
    }
}

fn build(fixture: Fixture, edits: [Vec<Edit>; 3]) -> Case {
    let start = &fixture_files()[&SpecPath::new(fixture.path)];
    let base = edited(fixture.top, start, Side::Base, &edits[0]);
    let ours = edited(fixture.top, &base, Side::Ours, &edits[1]);
    let theirs = edited(fixture.top, &base, Side::Theirs, &edits[2]);
    Case {
        fixture,
        base,
        ours,
        theirs,
        edits,
    }
}

pub fn cases() -> impl Strategy<Value = Case> {
    let edits = |most: usize| proptest::collection::vec(edit(), 0..=most);
    let growth = proptest::collection::vec(growth(), 0..=12);
    (0..FIXTURES.len(), growth, edits(6), edits(6))
        .prop_map(|(file, base, ours, theirs)| build(FIXTURES[file], [base, ours, theirs]))
}

/// Why the full document with the file at `path` replaced by `bytes` does not read, or is
/// not a formatter fixed point; none when it reads and is.
pub fn unsettled(path: &str, bytes: &[u8]) -> Option<String> {
    let mut files = fixture_files().clone();
    let path = SpecPath::new(path);
    files.insert(path.clone(), bytes.to_vec());
    if let Err(errors) = codec::read_tree(&files) {
        return Some(format!("does not read: {errors:?}"));
    }
    match loftspec::fmt::format(&files) {
        Err(errors) => Some(format!("does not format: {errors:?}")),
        Ok(formatted) => formatted.changed.get(&path).map(|changed| {
            format!(
                "the formatter rewrites it as\n{}",
                String::from_utf8_lossy(changed)
            )
        }),
    }
}
