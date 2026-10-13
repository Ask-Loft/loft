//! References written by name (Keys): once every key is minted, each reference the author wrote
//! as a record naming its target takes the key of the one element of an accepted kind that has
//! that name. An option and a driver are looked for in their own decision.

use crate::codec::schema::{Kind, field};
use crate::codec::{Problem, ReadError, Rec, SpecPath, Tree, Val, statement_of};

/// The kinds of record a node is written as.
const NODES: &[Kind] = &[
    Kind::Product,
    Kind::FeatureGroup,
    Kind::Feature,
    Kind::Requirement,
    Kind::Criterion,
    Kind::RootComponent,
    Kind::Component,
    Kind::Constraint,
    Kind::QualityAttribute,
    Kind::Section,
];

/// An element a name may identify.
#[derive(Debug, Clone)]
struct Named {
    kind: Kind,
    key: String,
    name: String,
}

/// The options and drivers of the decision a reference sits in.
struct Local {
    options: Vec<Named>,
    drivers: Vec<Named>,
}

/// Where a reference field looks for its target, and how an error names what it wanted.
enum Wanted {
    Nodes(&'static [Kind], &'static str),
    Elements,
    Decisions,
    Options,
    Drivers,
}

impl Wanted {
    fn of(kind: Kind, field: &str) -> Self {
        match (kind, field) {
            (Kind::Feature, field::REALIZES) => {
                Self::Nodes(&[Kind::Component, Kind::RootComponent], "Component")
            }
            (_, field::SCOPED_TO) => Self::Nodes(&[Kind::Product], "Product"),
            (_, field::SUCCESSOR) => Self::Decisions,
            (_, field::SELECTED_OPTION | field::OPTION) => Self::Options,
            (_, field::DRIVER) => Self::Drivers,
            (_, field::MENTIONS) => Self::Elements,
            _ => Self::Nodes(NODES, "node"),
        }
    }

    const fn describe(&self) -> &'static str {
        match self {
            Self::Nodes(_, what) => what,
            Self::Elements => "node or decision",
            Self::Decisions => "decision",
            Self::Options => "option of this decision",
            Self::Drivers => "driver of this decision",
        }
    }

    fn candidates<'a>(&self, index: &'a [Named], local: Option<&'a Local>) -> Vec<&'a Named> {
        let local = |pick: fn(&'a Local) -> &'a Vec<Named>| local.map(pick).into_iter().flatten();
        match self {
            Self::Nodes(kinds, _) => index.iter().filter(|n| kinds.contains(&n.kind)).collect(),
            Self::Elements => index.iter().collect(),
            Self::Decisions => index.iter().filter(|n| n.kind == Kind::Decision).collect(),
            Self::Options => local(|local| &local.options).collect(),
            Self::Drivers => local(|local| &local.drivers).collect(),
        }
    }
}

/// Replaces every reference written by name in the tree with its target's key.
///
/// # Errors
///
/// Each reference whose name matches no element of an accepted kind, or more than one.
pub(super) fn resolve(tree: &mut Tree) -> Result<(), Vec<ReadError>> {
    let mut index = Vec::new();
    for (_, content) in tree.files.values() {
        for record in content.records() {
            collect(record, &mut index);
        }
    }
    let mut errors = Vec::new();
    for (path, (_, content)) in &mut tree.files {
        let mut resolver = Resolver {
            index: &index,
            path,
            errors: &mut errors,
        };
        for record in content.records_mut() {
            resolver.record(record, None);
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// The nodes and decisions a record holds, itself included.
fn collect(record: &Rec, index: &mut Vec<Named>) {
    if (NODES.contains(&record.kind) || record.kind == Kind::Decision)
        && let Some(key) = record.text(field::KEY)
    {
        index.push(Named {
            kind: record.kind,
            key: key.to_owned(),
            name: name_of(record),
        });
    }
    for child in record.children() {
        collect(child, index);
    }
}

/// A node's name, a Requirement's statement as one sentence, or a decision's title.
fn name_of(record: &Rec) -> String {
    match record.kind {
        Kind::Requirement => record
            .record(field::STATEMENT)
            .map(|statement| statement_of(statement).render())
            .unwrap_or_default(),
        Kind::Decision => record.text(field::TITLE).unwrap_or_default().to_owned(),
        _ => record.text(field::NAME).unwrap_or_default().to_owned(),
    }
}

impl Local {
    /// A decision's options by title, and its drivers by label or by their subject's name.
    fn of(decision: &Rec, index: &[Named]) -> Self {
        let named = |record: &Rec, name: String| {
            Some(Named {
                kind: record.kind,
                key: record.text(field::KEY)?.to_owned(),
                name,
            })
        };
        let options = decision.records(field::OPTIONS).filter_map(|option| {
            named(
                option,
                option.text(field::TITLE).unwrap_or_default().to_owned(),
            )
        });
        let drivers = decision.records(field::DRIVERS).filter_map(|driver| {
            let name = match (driver.text(field::LABEL), driver.get(field::SUBJECT)) {
                (Some(label), _) => label.to_owned(),
                (None, Some(subject)) => subject_name(subject, index)?,
                (None, None) => return None,
            };
            named(driver, name)
        });
        Self {
            options: options.collect(),
            drivers: drivers.collect(),
        }
    }
}

/// The name of a subject driver's node: the name it was written by, or its node's name.
fn subject_name(subject: &Val, index: &[Named]) -> Option<String> {
    match subject {
        Val::Record(record) => record.text(field::NAME).map(str::to_owned),
        Val::Scalar { text, .. } => index
            .iter()
            .find(|n| &n.key == text)
            .map(|n| n.name.clone()),
        _ => None,
    }
}

/// The resolution of one file's references: the elements names may identify, the file's path,
/// and the errors found so far.
struct Resolver<'a> {
    index: &'a [Named],
    path: &'a SpecPath,
    errors: &'a mut Vec<ReadError>,
}

impl Resolver<'_> {
    /// Resolves the references a record and everything in it hold; `local` is the decision it
    /// sits in, if any.
    fn record(&mut self, record: &mut Rec, local: Option<&Local>) {
        let own;
        let local = if record.kind == Kind::Decision {
            own = Local::of(record, self.index);
            Some(&own)
        } else {
            local
        };
        let kind = record.kind;
        for (name, value) in &mut record.fields {
            self.field(value, &Wanted::of(kind, name), local);
        }
    }

    /// Resolves a field's value: one item, or each entry of a list.
    fn field(&mut self, value: &mut Val, wanted: &Wanted, local: Option<&Local>) {
        let items = match value {
            Val::List(items) => items.as_mut_slice(),
            single => std::slice::from_mut(single),
        };
        for item in items {
            self.item(item, wanted, local);
        }
    }

    /// Replaces a reference written by name with its target's key, or resolves inside a record.
    fn item(&mut self, item: &mut Val, wanted: &Wanted, local: Option<&Local>) {
        let Val::Record(record) = item else {
            return;
        };
        if record.kind != Kind::ByName {
            self.record(record, local);
            return;
        }
        match target(record, wanted, self.index, local) {
            Ok(key) => *item = key,
            Err(problem) => {
                let error = ReadError::at(self.path, Some(record.position), problem);
                self.errors.push(error);
            }
        }
    }
}

/// The key a reference by name takes, or why it takes none.
fn target(
    by_name: &Rec,
    wanted: &Wanted,
    index: &[Named],
    local: Option<&Local>,
) -> Result<Val, Problem> {
    let name = by_name.text(field::NAME).unwrap_or_default();
    let matches: Vec<&Named> = wanted
        .candidates(index, local)
        .into_iter()
        .filter(|candidate| candidate.name == name)
        .collect();
    match matches.as_slice() {
        [one] => Ok(Val::Scalar {
            text: one.key.clone(),
            position: by_name.position,
        }),
        [] => Err(Problem::NameUnresolved {
            name: name.to_owned(),
            wanted: wanted.describe(),
        }),
        many => Err(Problem::NameAmbiguous {
            name: name.to_owned(),
            keys: many.iter().map(|named| named.key.clone()).collect(),
        }),
    }
}
