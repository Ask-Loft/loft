//! The class of a path among the known trees, by its words alone.

pub use yardlog::consult::Class;

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
