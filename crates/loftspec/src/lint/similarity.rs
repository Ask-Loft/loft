//! `LQ-008`: two names of one kind that are equal, or whose multiset similarity reaches the
//! threshold; names shorter than the minimum take part only when equal (AC-399, AC-400, AC-644,
//! AC-645). The similarity of two names is the size of their tokens' multiset intersection over
//! the size of its union. A criterion means something only under its Requirement, and siblings
//! that differ in a word enumerate cases, so criteria are compared only with the other criteria
//! of their Requirement and only for identical tokens.

use std::collections::BTreeMap;

use super::context::Context;
use super::text::tokens;
use super::{LintConfig, Raw, RuleId, Similarity};
use crate::identity::Key;
use crate::ids::NodeId;
use crate::kind::NodeKind;

/// The names compared with one another: every node of a kind, or the criteria of one Requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Group {
    Kind(NodeKind),
    Criteria(Option<NodeId>),
}

/// A name as token numbers: in order, and sorted for counting.
struct Name {
    key: Key,
    tokens: Vec<u32>,
    sorted: Vec<u32>,
}

pub(super) fn check(context: &Context<'_>, config: &LintConfig, out: &mut Vec<Raw>) {
    let mut numbers = BTreeMap::<String, u32>::new();
    let mut groups = BTreeMap::<Group, Vec<Name>>::new();
    for item in &context.items {
        let kind = item.id.kind();
        let words = tokens(&item.name());
        if kind == NodeKind::Section || words.is_empty() {
            continue;
        }
        let tokens: Vec<u32> = words
            .into_iter()
            .map(|word| {
                let next = u32::try_from(numbers.len()).expect("fewer than 2^32 words");
                *numbers.entry(word).or_insert(next)
            })
            .collect();
        let mut sorted = tokens.clone();
        sorted.sort_unstable();
        let name = Name {
            key: item.id.key(),
            tokens,
            sorted,
        };
        let group = if kind == NodeKind::Criterion {
            Group::Criteria(item.parent)
        } else {
            Group::Kind(kind)
        };
        groups.entry(group).or_default().push(name);
    }
    let similarity = config.similarity();
    for (group, names) in &mut groups {
        let identical_only = matches!(group, Group::Criteria(_));
        names.sort_unstable_by_key(|name| name.key);
        for (index, left) in names.iter().enumerate() {
            let near = names[index + 1..].iter().filter(|right| {
                left.tokens == right.tokens || !identical_only && near(left, right, similarity)
            });
            out.extend(near.map(|right| {
                let message = "The names are near-duplicates within their kind.";
                Raw::new(RuleId::Lq008, vec![left.key, right.key], message)
            }));
        }
    }
}

/// Whether two names are equal, or long enough and as similar as the threshold asks.
fn near(left: &Name, right: &Name, similarity: Similarity) -> bool {
    if left.tokens == right.tokens {
        return true;
    }
    let (short, long) = if left.sorted.len() <= right.sorted.len() {
        (left.sorted.len(), right.sorted.len())
    } else {
        (right.sorted.len(), left.sorted.len())
    };
    // The intersection is at most the shorter name and the union at least the longer one.
    if short < similarity.min_tokens || !reaches(short, long, similarity.threshold) {
        return false;
    }
    let both = intersection(&left.sorted, &right.sorted);
    reaches(
        both,
        left.sorted.len() + right.sorted.len() - both,
        similarity.threshold,
    )
}

/// Whether `part / whole` reaches `threshold` millionths.
fn reaches(part: usize, whole: usize, threshold: u32) -> bool {
    (part as u128) * 1_000_000 >= (whole as u128) * u128::from(threshold)
}

/// The size of the multiset intersection of two sorted lists.
fn intersection(left: &[u32], right: &[u32]) -> usize {
    let (mut i, mut j, mut both) = (0, 0, 0);
    while i < left.len() && j < right.len() {
        match left[i].cmp(&right[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                both += 1;
                i += 1;
                j += 1;
            }
        }
    }
    both
}
