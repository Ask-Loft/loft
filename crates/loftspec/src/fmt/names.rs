//! References written by name (Keys): once every key is minted, each reference the author wrote
//! as a record naming its target takes the key of the one element of the field's [`Target`] that
//! has that name. An option and a driver are looked for in their own decision. Only the files
//! that hold such a reference are walked.

use std::collections::{BTreeSet, HashMap};

use crate::codec::schema::{Kind, Target, field};
use crate::codec::{Problem, ReadError, Rec, SpecPath, Tree, Val, keyed_records, statement_of};
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
        let files = tree.files.values();
        let records = keyed_records(files.flat_map(|(_, content)| content.records()));
        let mut index = Self {
            elements: Vec::new(),
            by_name: HashMap::new(),
            by_key: HashMap::new(),
        };
        for record in records {
            if let Some(key) = record.text(field::KEY)
                && (is_node(record.kind) || record.kind == Kind::Decision)
            {
                let at = index.elements.len();
                let name = name_of(record);
                index.by_name.entry(name.clone()).or_default().push(at);
                index.by_key.insert(key.to_owned(), at);
                index.elements.push(Named {
                    kind: record.kind,
                    key: key.to_owned(),
                    name,
                });
            }
        }
        index
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

/// The options and drivers of the decision a reference sits in.
struct Local {
    options: Keyed,
    drivers: Keyed,
}

const fn is_node(kind: Kind) -> bool {
    kind.node_kind().is_some()
}

/// Where a reference looks for its target.
enum Look {
    /// Among the tree's nodes and decisions, of the kinds the function accepts.
    Among(fn(Kind) -> bool),
    /// Among the options or the drivers of its own decision.
    Own(fn(&Local) -> &Keyed),
}

/// Where a reference to `target` looks.
fn look(target: Target) -> Look {
    match target {
        Target::Component => {
            Look::Among(|kind| matches!(kind.node_kind(), Some(NodeKind::Component)))
        }
        Target::Product => Look::Among(|kind| kind == Kind::Product),
        Target::Decision => Look::Among(|kind| kind == Kind::Decision),
        Target::NodeOrDecision => Look::Among(|kind| is_node(kind) || kind == Kind::Decision),
        Target::Option => Look::Own(|local| &local.options),
        Target::Driver => Look::Own(|local| &local.drivers),
        _ => Look::Among(is_node),
    }
}

/// How an error names what a reference to `target` wanted.
const fn describe(target: Target) -> &'static str {
    match target {
        Target::Component => "Component",
        Target::Product => "Product",
        Target::Decision => "decision",
        Target::NodeOrDecision => "node or decision",
        Target::Option => "option of this decision",
        Target::Driver => "driver of this decision",
        _ => "node",
    }
}

/// Replaces every reference written by name in the tree with its target's key.
///
/// # Errors
///
/// Each reference whose name matches no element of the field's target, or more than one.
pub(super) fn resolve(tree: &mut Tree) -> Result<(), Vec<ReadError>> {
    let written: BTreeSet<SpecPath> = tree
        .files
        .iter()
        .filter(|(_, (_, content))| content.records().any(holds_by_name))
        .map(|(path, _)| path.clone())
        .collect();
    if written.is_empty() {
        return Ok(());
    }
    let index = Index::of(tree);
    let mut errors = Vec::new();
    for (path, (_, content)) in &mut tree.files {
        if !written.contains(path) {
            continue;
        }
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
    /// sits in, gathered only for a decision that holds a reference by name.
    fn record(&mut self, record: &mut Rec, local: Option<&Local>) {
        let own;
        let local = if record.kind == Kind::Decision && holds_by_name(record) {
            own = Local::of(record, self.index);
            Some(&own)
        } else {
            local
        };
        let kind = record.kind;
        for (name, value) in &mut record.fields {
            let target = kind
                .field(name)
                .and_then(|(_, field)| field.shape.item().target());
            self.field(value, target, local);
        }
    }

    /// Resolves a field's value: one item, or each entry of a list.
    fn field(&mut self, value: &mut Val, target: Option<Target>, local: Option<&Local>) {
        let items = match value {
            Val::List(items) => items.as_mut_slice(),
            single => std::slice::from_mut(single),
        };
        for item in items {
            self.item(item, target, local);
        }
    }

    /// Replaces a reference written by name with its target's key, or resolves inside a record.
    /// The reader makes a reference by name only in a field whose item is a reference.
    fn item(&mut self, item: &mut Val, target: Option<Target>, local: Option<&Local>) {
        let Val::Record(record) = item else {
            return;
        };
        let (Kind::ByName, Some(target)) = (record.kind, target) else {
            self.record(record, local);
            return;
        };
        match self.target(record, target, local) {
            Ok(key) => *item = key,
            Err(problem) => {
                let error = ReadError::at(self.path, Some(record.position), problem);
                self.errors.push(error);
            }
        }
    }

    /// The key a reference by name takes, or why it takes none.
    fn target(&self, by_name: &Rec, target: Target, local: Option<&Local>) -> Result<Val, Problem> {
        let name = by_name.text(field::NAME).unwrap_or_default();
        let keys: Vec<&str> = match look(target) {
            Look::Among(accepts) => self.index.named(name, accepts),
            Look::Own(pick) => local
                .map(pick)
                .into_iter()
                .flatten()
                .filter(|(_, candidate)| candidate == name)
                .map(|(key, _)| key.as_str())
                .collect(),
        };
        match keys.as_slice() {
            [key] => Ok(Val::Scalar {
                text: (*key).to_owned(),
                position: by_name.position,
            }),
            [] => Err(Problem::NameUnresolved {
                name: name.to_owned(),
                wanted: describe(target),
            }),
            many => Err(Problem::NameAmbiguous {
                name: name.to_owned(),
                keys: many.iter().map(|key| (*key).to_owned()).collect(),
            }),
        }
    }
}
