//! The class of a path among the known trees, by its words alone.

/// What a path a tool call reads or edits is: the specification, the code, or neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Class {
    Spec,
    Code,
    Other,
}

impl Class {
    /// The class as an event names it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Spec => "spec",
            Self::Code => "code",
            Self::Other => "other",
        }
    }
}

/// The distinct classes of `classes`, in the order `spec`, `code`, `other`.
pub fn ordered(classes: impl IntoIterator<Item = Class>) -> Vec<Class> {
    let mut distinct: Vec<Class> = classes.into_iter().collect();
    distinct.sort_unstable();
    distinct.dedup();
    distinct
}

/// `path` made absolute against `dir`, with `.` and `..` resolved; none for a path from `~`.
pub fn resolve(path: &str, dir: &str) -> Option<String> {
    if path == "~" || path.starts_with("~/") {
        return None;
    }
    let joined = if path.starts_with('/') {
        path.to_owned()
    } else {
        format!("{dir}/{path}")
    };
    let mut parts: Vec<&str> = Vec::new();
    for part in joined.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    Some(format!("/{}", parts.join("/")))
}

/// Whether the tree `tree` holds the absolute path `path`.
pub fn holds(tree: &str, path: &str) -> bool {
    resolve(tree, "/").is_some_and(|root| within(path, &root).is_some())
}

/// `absolute` within the tree `root`, when the tree holds it: empty for the tree itself.
fn within<'a>(absolute: &'a str, root: &str) -> Option<&'a str> {
    if absolute == root {
        return Some("");
    }
    let prefix = if root == "/" { "" } else { root };
    absolute.strip_prefix(prefix)?.strip_prefix('/')
}

/// The class of `path`, named by an agent in `dir`, among the known `trees`.
pub fn class_of(path: &str, dir: &str, trees: &[String]) -> Class {
    let Some(absolute) = resolve(path, dir) else {
        return Class::Other;
    };
    // The nearest tree is the one that leaves the shortest path within it.
    let nearest = trees
        .iter()
        .filter_map(|tree| within(&absolute, &resolve(tree, "/")?).map(str::to_owned))
        .min_by_key(String::len);
    let Some(inside) = nearest else {
        return Class::Other;
    };
    match inside.split('/').next() {
        Some("spec") => Class::Spec,
        Some(".git" | ".yard") => Class::Other,
        _ => Class::Code,
    }
}
