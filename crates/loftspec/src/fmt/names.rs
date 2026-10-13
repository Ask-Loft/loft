//! References written by name (Keys): once every key is minted, each reference the author wrote
//! as a record naming its target takes the key of the one element of an accepted kind that has
//! that name. An option and a driver are looked for in their own decision. A tree without such a
//! reference costs one scan.

use std::collections::HashMap;

use crate::codec::schema::{Kind, field};
use crate::codec::{Problem, ReadError, Rec, SpecPath, Tree, Val, statement_of};
use crate::kind::NodeKind;

/// An element a name may identify.
struct Named {
    kind: Kind,
    key: String,
    name: String,
}

/// Every node and decision of the tree, by name and by key.
struct Index {
    elements: Vec<Named>,
    by_name: HashMap<String, Vec<usize>>,
    by_key: HashMap<String, usize>,
}

impl Index {
    fn of(tree: &Tree) -> Self {
        let mut elements = Vec::new();
        for (_, content) in tree.files.values() {
            for record in content.records() {
                collect(record, &mut elements);
            }
        }
        let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
        let mut by_key = HashMap::new();
        for (at, element) in elements.iter().enumerate() {
            by_name.entry(element.name.clone()).or_default().push(at);
            by_key.insert(element.key.clone(), at);
        }
        Self {
            elements,
            by_name,
            by_key,
        }
    }

    /// The keys of the elements named `name` whose kind `accepts` takes.
    fn named(&self, name: &str, accepts: fn(Kind) -> bool) -> Vec<&str> {
        self.by_name.get(name).map_or_else(Vec::new, |found| {
            found
                .iter()
                .map(|at| &self.elements[*at])
                .filter(|element| accepts(element.kind))
                .map(|element| element.key.as_str())
                .collect()
        })
    }
}

/// Elements of one decision, each as its key and name.
type Keyed = Vec<(String, String)>;

/// Which of a decision's elements a reference looks among.
type Pick = fn(&Local) -> &Keyed;

/// The options and drivers of the decision a reference sits in.
struct Local {
    options: Keyed,
    drivers: Keyed,
}

/// Where a reference field looks for its target, and how an error names what it wanted.
enum Wanted {
    Index(fn(Kind) -> bool, &'static str),
    Local(Pick, &'static str),
}

const fn is_node(kind: Kind) -> bool {
    kind.node_kind().is_some()
}

impl Wanted {
    fn of(kind: Kind, field: &str) -> Self {
        match (kind, field) {
            (Kind::Feature, field::REALIZES) => Self::Index(
                |kind| matches!(kind.node_kind(), Some(NodeKind::Component)),
                "Component",
            ),
            (_, field::SCOPED_TO) => Self::Index(|kind| kind == Kind::Product, "Product"),
            (_, field::SUCCESSOR) => Self::Index(|kind| kind == Kind::Decision, "decision"),
            (_, field::SELECTED_OPTION | field::OPTION) => {
                Self::Local(|local| &local.options, "option of this decision")
            }
            (_, field::DRIVER) => Self::Local(|local| &local.drivers, "driver of this decision"),
            (_, field::MENTIONS) => Self::Index(
                |kind| is_node(kind) || kind == Kind::Decision,
                "node or decision",
            ),
            _ => Self::Index(is_node, "node"),
        }
    }

    const fn describe(&self) -> &'static str {
        match self {
            Self::Index(_, what) | Self::Local(_, what) => what,
        }
    }
}

/// Replaces every reference written by name in the tree with its target's key.
///
/// # Errors
///
/// Each reference whose name matches no element of an accepted kind, or more than one.
pub(super) fn resolve(tree: &mut Tree) -> Result<(), Vec<ReadError>> {
    let written = tree
        .files
        .values()
        .any(|(_, content)| content.records().any(holds_by_name));
    if !written {
        return Ok(());
    }
    let index = Index::of(tree);
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

/// Whether a record, or any record in it, is a reference written by name.
fn holds_by_name(record: &Rec) -> bool {
    record.kind == Kind::ByName || record.children().any(holds_by_name)
}

/// The nodes and decisions a record holds, itself included.
fn collect(record: &Rec, elements: &mut Vec<Named>) {
    if (is_node(record.kind) || record.kind == Kind::Decision)
        && let Some(key) = record.text(field::KEY)
    {
        elements.push(Named {
            kind: record.kind,
            key: key.to_owned(),
            name: name_of(record),
        });
    }
    for child in record.children() {
        collect(child, elements);
    }
}

/// A record's name as [`crate::node::Node::name`] gives a node's, or a decision's title: a
/// Requirement's is its statement as one sentence.
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
    fn of(decision: &Rec, index: &Index) -> Self {
        let keyed =
            |record: &Rec, name: Option<String>| Some((record.text(field::KEY)?.to_owned(), name?));
        let options = decision
            .records(field::OPTIONS)
            .filter_map(|option| keyed(option, option.text(field::TITLE).map(str::to_owned)));
        let drivers = decision.records(field::DRIVERS).filter_map(|driver| {
            let name = match (driver.text(field::LABEL), driver.get(field::SUBJECT)) {
                (Some(label), _) => Some(label.to_owned()),
                (None, Some(subject)) => subject_name(subject, index),
                (None, None) => None,
            };
            keyed(driver, name)
        });
        Self {
            options: options.collect(),
            drivers: drivers.collect(),
        }
    }

    /// The keys of the options or drivers `pick` gives that are named `name`.
    fn named<'a>(&'a self, pick: Pick, name: &str) -> Vec<&'a str> {
        pick(self)
            .iter()
            .filter(|(_, candidate)| candidate == name)
            .map(|(key, _)| key.as_str())
            .collect()
    }
}

/// The name of a subject driver's node: the name it was written by, or its node's name.
fn subject_name(subject: &Val, index: &Index) -> Option<String> {
    match subject {
        Val::Record(record) => record.text(field::NAME).map(str::to_owned),
        Val::Scalar { text, .. } => index
            .by_key
            .get(text)
            .map(|at| index.elements[*at].name.clone()),
        _ => None,
    }
}

/// The resolution of one file's references: the elements names may identify, the file's path,
/// and the errors found so far.
struct Resolver<'a> {
    index: &'a Index,
    path: &'a SpecPath,
    errors: &'a mut Vec<ReadError>,
}

impl Resolver<'_> {
    /// Resolves the references a record and everything in it hold; `local` is the decision it
    /// sits in, if any. A record that holds none is passed by.
    fn record(&mut self, record: &mut Rec, local: Option<&Local>) {
        if !holds_by_name(record) {
            return;
        }
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
        match self.target(record, wanted, local) {
            Ok(key) => *item = key,
            Err(problem) => {
                let error = ReadError::at(self.path, Some(record.position), problem);
                self.errors.push(error);
            }
        }
    }

    /// The key a reference by name takes, or why it takes none.
    fn target(
        &self,
        by_name: &Rec,
        wanted: &Wanted,
        local: Option<&Local>,
    ) -> Result<Val, Problem> {
        let name = by_name.text(field::NAME).unwrap_or_default();
        let keys = match wanted {
            Wanted::Index(accepts, _) => self.index.named(name, *accepts),
            Wanted::Local(pick, _) => local.map_or_else(Vec::new, |local| local.named(*pick, name)),
        };
        match keys.as_slice() {
            [key] => Ok(Val::Scalar {
                text: (*key).to_owned(),
                position: by_name.position,
            }),
            [] => Err(Problem::NameUnresolved {
                name: name.to_owned(),
                wanted: wanted.describe(),
            }),
            many => Err(Problem::NameAmbiguous {
                name: name.to_owned(),
                keys: many.iter().map(|key| (*key).to_owned()).collect(),
            }),
        }
    }
}
