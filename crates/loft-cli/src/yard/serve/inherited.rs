//! A repository's inherited Yard Logs, as the menu lists them: each Yard Log once, however many
//! of the repository's trees hold it, and none that one of its trees is on; the one closed last
//! first, or opened last where none is closed; filtered by branch or title whatever its case; and
//! given 50 at a time, for the listing carries only their count.

use std::collections::{HashMap, HashSet};

use serde_json::{Value, json};

/// How many inherited Yard Logs a page gives.
pub const PAGE: usize = 50;

/// An inherited Yard Log as one tree holds it: its key (its branch and tree where it could not
/// be read), whether that tree is the repository's main worktree, and its entry in the list.
pub struct Inherited {
    pub key: String,
    pub main: bool,
    pub entry: Value,
}

/// How many Yard Logs [`distinct`] gives of `found`, without choosing or ordering them.
pub fn distinct_count(found: &[Inherited], current: &HashSet<String>) -> usize {
    found
        .iter()
        .map(|each| each.key.as_str())
        .filter(|key| !current.contains(*key))
        .collect::<HashSet<_>>()
        .len()
}

/// Each Yard Log of `found` once, from the main worktree where that holds it and from the first
/// tree that does otherwise, less those whose key `current` names; the newest first.
pub fn distinct(found: Vec<Inherited>, current: &HashSet<String>) -> Vec<Inherited> {
    let mut kept: Vec<Inherited> = Vec::new();
    let mut at: HashMap<String, usize> = HashMap::new();
    for each in found {
        if current.contains(&each.key) {
            continue;
        }
        match at.get(&each.key) {
            Some(&held) if each.main && !kept[held].main => kept[held] = each,
            Some(_) => {}
            None => {
                at.insert(each.key.clone(), kept.len());
                kept.push(each);
            }
        }
    }
    kept.sort_by(|a, b| {
        newest(&b.entry)
            .cmp(newest(&a.entry))
            .then_with(|| branch(&a.entry).cmp(branch(&b.entry)))
    });
    kept
}

/// When the Yard Log was closed, or opened where it is not closed.
fn newest(entry: &Value) -> &str {
    entry["closedAt"]
        .as_str()
        .or_else(|| entry["openedAt"].as_str())
        .unwrap_or_default()
}

fn branch(entry: &Value) -> &str {
    entry["branch"].as_str().unwrap_or_default()
}

/// The page of `list` from `from` of those whose branch or title holds `filter`, whatever its
/// case, with how many there are.
pub fn page(list: &[Inherited], filter: &str, from: usize) -> Value {
    let wanted = filter.trim().to_lowercase();
    let matching: Vec<&Value> = list
        .iter()
        .map(|each| &each.entry)
        .filter(|entry| {
            wanted.is_empty()
                || [branch(entry), entry["title"].as_str().unwrap_or_default()]
                    .iter()
                    .any(|text| text.to_lowercase().contains(&wanted))
        })
        .collect();
    let logs: Vec<&Value> = matching.iter().skip(from).take(PAGE).copied().collect();
    json!({ "total": matching.len(), "from": from, "logs": logs })
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use serde_json::json;

    use super::{Inherited, PAGE, distinct, page};

    fn held(key: &str, tree: &str, main: bool, closed: Option<&str>) -> Inherited {
        Inherited {
            key: key.to_owned(),
            main,
            entry: json!({
                "branch": format!("feat/{key}"),
                "title": format!("Title of {key}"),
                "tree": tree,
                "openedAt": "2026-01-01T00:00:00Z",
                "closedAt": closed,
            }),
        }
    }

    #[test]
    fn a_yard_log_held_by_more_than_one_tree_is_listed_once_from_the_main_worktree() {
        let found = vec![
            held("a", "/work/one", false, None),
            held("a", "/work/main", true, None),
            held("a", "/work/two", false, None),
            held("b", "/work/one", false, None),
            held("b", "/work/two", false, None),
        ];
        let list = distinct(found, &HashSet::new());
        let trees: Vec<_> = list
            .iter()
            .map(|each| (each.key.as_str(), each.entry["tree"].as_str().unwrap()))
            .collect();
        assert_eq!(trees, [("a", "/work/main"), ("b", "/work/one")]);
    }

    #[test]
    fn a_yard_log_a_tree_is_on_is_not_among_the_inherited() {
        let found = vec![
            held("a", "/work/one", false, None),
            held("b", "/work/one", false, None),
        ];
        let list = distinct(found, &HashSet::from(["a".to_owned()]));
        assert_eq!(
            list.iter()
                .map(|each| each.key.as_str())
                .collect::<Vec<_>>(),
            ["b"]
        );
    }

    #[test]
    fn the_yard_log_closed_last_comes_first_or_opened_last_where_none_is_closed() {
        let mut open = held("open", "/t", false, None);
        open.entry["openedAt"] = json!("2026-03-01T00:00:00Z");
        let found = vec![
            held("old", "/t", false, Some("2026-01-02T00:00:00Z")),
            open,
            held("new", "/t", false, Some("2026-02-01T00:00:00Z")),
        ];
        let order: Vec<_> = distinct(found, &HashSet::new())
            .into_iter()
            .map(|each| each.key)
            .collect();
        assert_eq!(order, ["open", "new", "old"]);
    }

    #[test]
    fn the_inherited_yard_logs_are_given_50_at_a_time_and_filtered_by_branch_or_title_whatever_the_case()
     {
        let found: Vec<_> = (0..=PAGE)
            .map(|n| held(&format!("k{n:03}"), "/t", false, None))
            .collect();
        let list = distinct(found, &HashSet::new());
        let first = page(&list, "", 0);
        assert_eq!(
            (
                first["total"].as_u64(),
                first["logs"].as_array().map(Vec::len)
            ),
            (Some(51), Some(PAGE))
        );
        assert_eq!(
            page(&list, "", PAGE)["logs"].as_array().map(Vec::len),
            Some(1)
        );
        let by_branch = page(&list, "FEAT/K007", 0);
        assert_eq!(by_branch["total"], 1);
        assert_eq!(by_branch["logs"][0]["branch"], "feat/k007");
        assert_eq!(page(&list, "title of k01", 0)["total"], 10);
    }
}
