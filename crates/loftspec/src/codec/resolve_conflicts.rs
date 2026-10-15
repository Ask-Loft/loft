//! Conflict records between the tree and the model: collected from every record that holds a
//! `conflicts` list when the tree is resolved, and attached back to those records when the
//! document is written, each side in the shape the field it names takes (AC-860).

use std::collections::BTreeMap;

use super::conflict::{SideShape, side_shape};
use super::files::Tree;
use super::record::{ConflictRec, Rec, Val};
use super::schema::{Item, Kind, Shape, field};
use crate::conflict::{Conflict, ConflictOwner, ConflictValue, FieldPath};
use crate::identity::Key;
use crate::ids::{
    CategoryId, ConsequenceId, DecisionId, DriverId, ElementId, NodeId, OptionId, TagId,
};

/// Every conflict the tree holds, in file order and then in the order each record lists them.
pub(super) fn collect(tree: &Tree) -> Vec<Conflict> {
    let mut conflicts = Vec::new();
    for (_, content) in tree.files.values() {
        for record in content.records() {
            collect_from(record, &mut conflicts);
        }
    }
    conflicts
}

fn collect_from(record: &Rec, conflicts: &mut Vec<Conflict>) {
    if let Some(owner) = owner(record) {
        for conflict in &record.conflicts {
            let [base, ours, theirs] = conflict.sides().map(|(_, side)| side.map(value));
            conflicts.push(Conflict {
                owner,
                field: FieldPath::new(&conflict.field),
                base,
                ours,
                theirs,
            });
        }
    }
    for child in record.children() {
        collect_from(child, conflicts);
    }
}

/// The owner a record that may hold conflicts is, by its kind and its key.
fn owner(record: &Rec) -> Option<ConflictOwner> {
    let key = || Key::parse(record.text(field::KEY)?).ok();
    Some(match (record.kind, record.kind.node_kind()) {
        (Kind::Manifest, _) => ConflictOwner::Manifest,
        (Kind::Labels, _) => ConflictOwner::Labels,
        (_, Some(kind)) => ConflictOwner::Element(ElementId::Node(NodeId::new(kind, key()?))),
        (Kind::Decision, _) => ConflictOwner::Element(ElementId::Decision(DecisionId::new(key()?))),
        (Kind::Option, _) => ConflictOwner::Option(OptionId::new(key()?)),
        (Kind::Driver, _) => ConflictOwner::Driver(DriverId::new(key()?)),
        (Kind::Consequence, _) => ConflictOwner::Consequence(ConsequenceId::new(key()?)),
        (Kind::TagEntry, _) => ConflictOwner::Tag(TagId::new(key()?)),
        (Kind::CategoryEntry, _) => ConflictOwner::Category(CategoryId::new(key()?)),
        _ => return None,
    })
}

fn value(val: &Val) -> ConflictValue {
    match val {
        Val::Null { .. } => ConflictValue::Deleted,
        Val::Scalar { text, .. } => ConflictValue::Scalar(text.clone()),
        Val::Ext { value, .. } => ConflictValue::Ext(value.clone()),
        Val::List(items) => ConflictValue::List(items.iter().map(value).collect()),
        Val::Record(record) => ConflictValue::Record(
            record
                .fields
                .iter()
                .map(|(name, inner)| ((*name).to_owned(), value(inner)))
                .collect(),
        ),
    }
}

/// Gives each record of the tree that holds conflicts its `conflicts` list.
pub(super) fn attach(tree: &mut Tree, conflicts: &[Conflict]) {
    if conflicts.is_empty() {
        return;
    }
    let mut by_owner: BTreeMap<ConflictOwner, Vec<&Conflict>> = BTreeMap::new();
    for conflict in conflicts {
        by_owner.entry(conflict.owner).or_default().push(conflict);
    }
    for (_, content) in tree.files.values_mut() {
        for record in content.records_mut() {
            attach_to(record, &by_owner);
        }
    }
}

fn attach_to(record: &mut Rec, by_owner: &BTreeMap<ConflictOwner, Vec<&Conflict>>) {
    if let Some(list) = owner(record).and_then(|owner| by_owner.get(&owner)) {
        let kind = record.kind;
        record.conflicts = list.iter().map(|conflict| rec(conflict, kind)).collect();
    }
    for child in record.children_mut() {
        attach_to(child, by_owner);
    }
}

/// A conflict as the record its host holds.
fn rec(conflict: &Conflict, host: Kind) -> ConflictRec {
    let shape = side_shape(host, conflict.field.as_str()).expect("a conflict the reader checked");
    let sides = [&conflict.base, &conflict.ours, &conflict.theirs];
    ConflictRec::new(
        conflict.field.as_str(),
        sides.map(|side| side.as_ref().map(|side| side_val(side, shape))),
    )
}

fn side_val(side: &ConflictValue, shape: SideShape) -> Val {
    match (shape, side) {
        (_, ConflictValue::Deleted) => Val::Null {
            position: crate::yaml::Position::new(1, 1),
        },
        (_, ConflictValue::Ext(value)) => Val::Ext {
            value: value.clone(),
            position: crate::yaml::Position::new(1, 1),
        },
        (SideShape::Field(Shape::List(item)), ConflictValue::List(items)) => {
            Val::List(items.iter().map(|each| item_val(each, item)).collect())
        }
        (SideShape::Field(Shape::One(item)), _) => item_val(side, item),
        _ => scalar_val(side),
    }
}

fn item_val(value: &ConflictValue, item: Item) -> Val {
    match (item, value) {
        (Item::Record(kind), ConflictValue::Record(fields)) => record_val(kind, fields),
        (Item::NodeRef, ConflictValue::Record(fields)) => record_val(Kind::FormerLabel, fields),
        _ => scalar_val(value),
    }
}

fn record_val(kind: Kind, fields: &[(String, ConflictValue)]) -> Val {
    let mut record = Rec::new(kind);
    for (name, inner) in fields {
        let (_, field) = kind.field(name).expect("a field the reader checked");
        record.set(field.name, side_val(inner, SideShape::Field(field.shape)));
    }
    Val::Record(record)
}

fn scalar_val(value: &ConflictValue) -> Val {
    match value {
        ConflictValue::Scalar(text) => Val::scalar(text),
        _ => unreachable!("a scalar the reader checked"),
    }
}
