//! The files of a `spec/` tree, each read as the record its path names (F-6), and written
//! back in canonical layout.

use std::collections::BTreeMap;

use super::error::{Problem, ReadError, SpecPath};
use super::record::{Mode, Rec, check, to_out};
use super::schema::Kind;
use crate::yaml::{self, Out, Value};

/// The bytes of each file under `spec/`, by path.
pub type Files = BTreeMap<SpecPath, Vec<u8>>;

pub const MANIFEST: &str = "loftspec.yaml";
pub const LABELS: &str = "labels.yaml";
pub const TAGS: &str = "tags.yaml";
pub const CATEGORIES: &str = "categories.yaml";
pub const TERMS: &str = "terms.yaml";
pub const LINT: &str = "lint.yaml";

/// What a file is, from its path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    Manifest,
    Labels,
    Tags,
    Categories,
    Terms,
    Lint,
    Product {
        product: String,
    },
    Feature {
        product: String,
        group: String,
        feature: String,
    },
    Component {
        slug: String,
    },
    Decision {
        slug: String,
    },
}

impl Role {
    /// The role of a path, or none for a path the Format does not name (AC-93).
    #[must_use]
    pub fn of(path: &SpecPath) -> Option<Self> {
        let parts: Vec<&str> = path.parts().collect();
        let stem = |name: &str| name.strip_suffix(".yaml").map(str::to_owned);
        Some(match parts.as_slice() {
            [MANIFEST] => Self::Manifest,
            [LABELS] => Self::Labels,
            [TAGS] => Self::Tags,
            [CATEGORIES] => Self::Categories,
            [TERMS] => Self::Terms,
            [LINT] => Self::Lint,
            ["products", product, "product.yaml"] => Self::Product {
                product: (*product).to_owned(),
            },
            ["products", product, group, feature] => Self::Feature {
                product: (*product).to_owned(),
                group: (*group).to_owned(),
                feature: stem(feature)?,
            },
            ["components", component] => Self::Component {
                slug: stem(component)?,
            },
            ["decisions", decision] => Self::Decision {
                slug: stem(decision)?,
            },
            _ => return None,
        })
    }

    /// The path of a file of this role: the inverse of [`Role::of`].
    #[must_use]
    pub fn path(&self) -> SpecPath {
        SpecPath::new(&match self {
            Self::Manifest => MANIFEST.to_owned(),
            Self::Labels => LABELS.to_owned(),
            Self::Tags => TAGS.to_owned(),
            Self::Categories => CATEGORIES.to_owned(),
            Self::Terms => TERMS.to_owned(),
            Self::Lint => LINT.to_owned(),
            Self::Product { product } => format!("products/{product}/product.yaml"),
            Self::Feature {
                product,
                group,
                feature,
            } => format!("products/{product}/{group}/{feature}.yaml"),
            Self::Component { slug } => format!("components/{slug}.yaml"),
            Self::Decision { slug } => format!("decisions/{slug}.yaml"),
        })
    }

    /// The file or directory name the file's record's slug must equal (RQ-211).
    #[must_use]
    pub fn slug(&self) -> Option<&str> {
        match self {
            Self::Product { product } => Some(product),
            Self::Feature { feature, .. } => Some(feature),
            Self::Component { slug } | Self::Decision { slug } => Some(slug),
            _ => None,
        }
    }

    /// Whether the writer writes a file of this role from the document: every role but the
    /// lint configuration, which the document does not hold.
    #[must_use]
    pub const fn is_written(&self) -> bool {
        self.kind().is_some()
    }

    /// The record kind the file holds at its top level; none for the lint configuration,
    /// which stage 5 reads.
    const fn kind(&self) -> Option<Kind> {
        match self {
            Self::Manifest => Some(Kind::Manifest),
            Self::Labels => Some(Kind::Labels),
            Self::Tags => Some(Kind::TagEntry),
            Self::Categories => Some(Kind::CategoryEntry),
            Self::Terms => Some(Kind::TermEntry),
            Self::Lint => None,
            Self::Product { .. } => Some(Kind::Product),
            Self::Feature { .. } => Some(Kind::Feature),
            Self::Component { .. } => Some(Kind::RootComponent),
            Self::Decision { .. } => Some(Kind::Decision),
        }
    }

    /// Whether the file holds a sequence of records rather than one.
    pub(crate) const fn is_registry(&self) -> bool {
        matches!(self, Self::Tags | Self::Categories | Self::Terms)
    }
}

/// One file's content: its records, or for the lint configuration its bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Content {
    Record(Rec),
    Registry(Vec<Rec>),
    Lint(Vec<u8>),
}

impl Content {
    /// The records the file holds at its top level.
    pub fn records(&self) -> impl Iterator<Item = &Rec> {
        let records: &[Rec] = match self {
            Self::Record(record) => std::slice::from_ref(record),
            Self::Registry(entries) => entries,
            Self::Lint(_) => &[],
        };
        records.iter()
    }

    /// The records the file holds at its top level, to change.
    pub fn records_mut(&mut self) -> impl Iterator<Item = &mut Rec> {
        let records: &mut [Rec] = match self {
            Self::Record(record) => std::slice::from_mut(record),
            Self::Registry(entries) => entries,
            Self::Lint(_) => &mut [],
        };
        records.iter_mut()
    }
}

/// Every file of a tree, checked, by path.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Tree {
    pub files: BTreeMap<SpecPath, (Role, Content)>,
}

impl Tree {
    /// The record a path holds.
    #[must_use]
    pub fn record(&self, path: &SpecPath) -> Option<&Rec> {
        match self.files.get(path) {
            Some((_, Content::Record(record))) => Some(record),
            _ => None,
        }
    }

    /// The records of each file of one role, in path order.
    pub fn records<'a>(
        &'a self,
        wanted: impl Fn(&Role) -> bool + 'a,
    ) -> impl Iterator<Item = (&'a SpecPath, &'a Role, &'a Rec)> + 'a {
        self.files
            .iter()
            .filter(move |(_, (role, _))| wanted(role))
            .filter_map(|(path, (role, content))| match content {
                Content::Record(record) => Some((path, role, record)),
                _ => None,
            })
    }

    /// The entries of a registry file; none when the file is absent.
    #[must_use]
    pub fn registry(&self, name: &str) -> &[Rec] {
        match self.files.get(&SpecPath::new(name)) {
            Some((_, Content::Registry(entries))) => entries,
            _ => &[],
        }
    }
}

/// Reads one file as the record or registry its path names.
pub(crate) fn read_file(
    path: &SpecPath,
    bytes: &[u8],
    mode: Mode,
) -> Result<(Role, Content), ReadError> {
    let role = Role::of(path).ok_or_else(|| ReadError::at(path, None, Problem::UnknownFile))?;
    let node = yaml::read(bytes).map_err(|error| ReadError::yaml(path, error))?;
    let Some(kind) = role.kind() else {
        return Ok((role, Content::Lint(bytes.to_vec())));
    };
    let content = match node {
        None if role.is_registry() => Content::Registry(Vec::new()),
        None => {
            return Err(ReadError::at(
                path,
                None,
                Problem::Expected("a record; the file is empty"),
            ));
        }
        Some(node) if role.is_registry() => {
            let Value::Sequence(items) = &node.value else {
                return Err(ReadError::at(
                    path,
                    Some(node.position),
                    Problem::Expected("a list of entries"),
                ));
            };
            let entries = items
                .iter()
                .map(|item| check(item, kind, mode))
                .collect::<Result<_, _>>()
                .map_err(|located| located.into_error(path))?;
            Content::Registry(entries)
        }
        Some(node) => {
            Content::Record(check(&node, kind, mode).map_err(|located| located.into_error(path))?)
        }
    };
    Ok((role, content))
}

/// Each file of the tree in canonical layout.
#[must_use]
pub fn write_files(tree: &Tree) -> Files {
    tree.files
        .iter()
        .map(|(path, (_, content))| {
            let bytes = match content {
                Content::Record(record) => yaml::write(&to_out(record)).into_bytes(),
                Content::Registry(entries) => {
                    yaml::write(&Out::Sequence(entries.iter().map(to_out).collect())).into_bytes()
                }
                Content::Lint(bytes) => bytes.clone(),
            };
            (path.clone(), bytes)
        })
        .collect()
}

/// Every keyed record of the record trees `roots`, with its key when it has one, depth first.
pub(crate) fn keyed_records<'a>(roots: impl IntoIterator<Item = &'a Rec>) -> Vec<&'a Rec> {
    let mut found = Vec::new();
    for root in roots {
        keyed(root, &mut found);
    }
    found
}

fn keyed<'a>(record: &'a Rec, found: &mut Vec<&'a Rec>) {
    if record.kind.is_keyed() {
        found.push(record);
    }
    // A conflict's sides hold no keyed record (`side_shape`), and conflicts are no child.
    for child in record.children() {
        keyed(child, found);
    }
}

/// The text of a tree's `spec/lint.yaml`; none for a tree without the file.
///
/// # Errors
///
/// The file is not UTF-8: the error names it and the byte offset of the first invalid sequence,
/// as the reader names it (RQ-65).
pub fn lint_configuration(files: &Files) -> Result<Option<&str>, ReadError> {
    let path = SpecPath::new(LINT);
    let Some(bytes) = files.get(&path) else {
        return Ok(None);
    };
    (yaml::utf8(bytes).map(Some)).map_err(|error| ReadError::yaml(&path, error))
}
