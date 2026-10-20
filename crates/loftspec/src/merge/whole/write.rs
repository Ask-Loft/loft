//! The merged records built back into the files of a tree: each record file's top record in the
//! file that its merged parent and slug place it in, and the lists of keys that place Products
//! and Features, the manifest's and each Feature Group's, holding the records the merge put
//! there.

use std::collections::{BTreeMap, BTreeSet};

use super::super::flat::{
    Build, COMPONENT_FILES, DECISION_FILES, FEATURE_FILES, Flat, PRODUCT_FILES, Parent, Place,
};
use super::super::tree::ByKey;
use super::TreeMergeError;
use super::sides::listed;
use crate::codec::{Content, MANIFEST, Rec, Role, SpecPath, Tree, Val, canonicalize, field};
use crate::identity::Key;

/// The merged tree in canonical layout: the single files and every record file.
///
/// # Errors
///
/// Two records that their parents and slugs place in one file.
pub(super) fn tree(
    singles: BTreeMap<SpecPath, (Role, Content)>,
    merged: ByKey,
    flats: &[Flat; 3],
) -> Result<Tree, TreeMergeError> {
    let mut tree = Tree { files: singles };
    let mut writer = Writer {
        build: Build::new(merged.records, &merged.lists),
        lists: &merged.lists,
        flats,
        tree: &mut tree,
    };
    let products = writer.products();
    for product in products {
        writer.product(product)?;
    }
    for key in writer.listed(&(Parent::Top, COMPONENT_FILES)) {
        writer.file(key, |slug| Role::Component {
            slug: slug.to_owned(),
        })?;
    }
    for key in writer.listed(&(Parent::Top, DECISION_FILES)) {
        writer.file(key, |slug| Role::Decision {
            slug: slug.to_owned(),
        })?;
    }
    canonicalize(&mut tree);
    Ok(tree)
}

struct Writer<'a> {
    build: Build<'a>,
    lists: &'a BTreeMap<Place, Vec<Key>>,
    flats: &'a [Flat; 3],
    tree: &'a mut Tree,
}

impl Writer<'_> {
    fn listed(&self, place: &Place) -> Vec<Key> {
        self.lists.get(place).cloned().unwrap_or_default()
    }

    /// The Products the merge keeps, in the order of the merged manifest's `products`, which
    /// is set to them.
    fn products(&mut self) -> Vec<Key> {
        let place = (Parent::Top, PRODUCT_FILES);
        let members = self.listed(&place);
        let path = SpecPath::new(MANIFEST);
        let Some((_, Content::Record(manifest))) = self.tree.files.get_mut(&path) else {
            return members;
        };
        let order = placed(
            &listed(manifest, field::PRODUCTS),
            &members,
            sides(self.flats, &place),
        );
        set_keys(manifest, field::PRODUCTS, &order);
        order
    }

    /// A Product's file and the files of the Features its Feature Groups list, each group's
    /// `features` set to the Features the merge put in it.
    fn product(&mut self, product: Key) -> Result<(), TreeMergeError> {
        let product_slug = self.slug(product);
        for group in self.listed(&(Parent::Key(product), field::FEATURE_GROUPS)) {
            let place = (Parent::Key(group), FEATURE_FILES);
            let members = self.listed(&place);
            let sides = sides(self.flats, &place);
            let Some(record) = self.build.get_mut(group) else {
                continue;
            };
            let order = placed(&listed(record, field::FEATURES), &members, sides);
            set_keys(record, field::FEATURES, &order);
            let group_slug = self.slug(group);
            for feature in order {
                self.file(feature, |slug| Role::Feature {
                    product: product_slug.clone(),
                    group: group_slug.clone(),
                    feature: slug.to_owned(),
                })?;
            }
        }
        self.file(product, |slug| Role::Product {
            product: slug.to_owned(),
        })
    }

    fn slug(&mut self, key: Key) -> String {
        self.build
            .get_mut(key)
            .and_then(|record| record.text(field::SLUG))
            .unwrap_or_default()
            .to_owned()
    }

    /// The file of the record keyed `key`, at the path its slug gives the role `role` names.
    fn file(&mut self, key: Key, role: impl FnOnce(&str) -> Role) -> Result<(), TreeMergeError> {
        let role = role(&self.slug(key));
        let Some(record) = self.build.take(key) else {
            return Ok(());
        };
        let path = role.path();
        if self.tree.files.contains_key(&path) {
            return Err(TreeMergeError::SamePath(path));
        }
        self.tree
            .files
            .insert(path, (role, Content::Record(record)));
        Ok(())
    }
}

/// Each side's list at `place`.
fn sides<'f>(flats: &'f [Flat; 3], place: &Place) -> [&'f [Key]; 3] {
    flats
        .each_ref()
        .map(|flat| flat.lists.get(place).map_or(&[][..], Vec::as_slice))
}

/// The keys of a list that places records of their own files, `members`, in order: as the
/// merged list orders them, and a member it lacks after the member it follows in the first
/// version that holds it in this list, base first, or first.
fn placed(merged: &[Key], members: &[Key], sides: [&[Key]; 3]) -> Vec<Key> {
    let wanted: BTreeSet<&Key> = members.iter().collect();
    let mut seen = BTreeSet::new();
    let mut order: Vec<Key> = merged
        .iter()
        .filter(|key| wanted.contains(key) && seen.insert(**key))
        .copied()
        .collect();
    for key in members.iter().filter(|key| !seen.contains(key)) {
        let after = sides.iter().find_map(|side| {
            let at = side.iter().position(|each| each == key)?;
            Some(
                side[..at]
                    .iter()
                    .rev()
                    .find_map(|before| order.iter().position(|each| each == before)),
            )
        });
        let at = after.flatten().map_or(0, |before| before + 1);
        order.insert(at, *key);
    }
    order
}

/// Sets a list field of keys, left out when it holds none.
fn set_keys(record: &mut Rec, name: &'static str, keys: &[Key]) {
    if keys.is_empty() {
        record.fields.retain(|(field, _)| *field != name);
    } else {
        record.set(name, Val::List(keys.iter().map(Val::scalar).collect()));
    }
}
