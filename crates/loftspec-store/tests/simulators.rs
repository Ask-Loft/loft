//! The simulators of the Software Plant: each Feature that lists `plant-simulator` mentions the
//! one external Component it stands in for and has a contract suite, and each contract suite
//! belongs to such a Feature.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use loftspec::boundary::Boundaries;
use loftspec::document::{Document, Edge};
use loftspec::ids::{ComponentId, ElementId, NodeId};
use loftspec::node::Node;
use loftspec::tree::Feature;
use loftspec_store::{DocumentStore, FileStore};

const TAG: &str = "plant-simulator";

/// The slug of the Software Plant.
const PLANT: &str = "software-plant";

/// The contract suite of each simulator, by the slug of its Feature: a test file of the crate
/// whose port reaches the real system, by its path in the repository.
const SUITES: [(&str, &str); 4] = [
    (
        "git-simulator",
        "crates/loftspec-store/tests/git_contract.rs",
    ),
    (
        "clock-simulator",
        "crates/loftspec-store/tests/clock_contract.rs",
    ),
    (
        "collector-simulator",
        "crates/loft-cli/tests/collector_contract.rs",
    ),
    (
        "release-feed-simulator",
        "crates/loft-cli/tests/release_feed_contract.rs",
    ),
];

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn document() -> Document {
    FileStore::open(&repository()).unwrap().load().unwrap()
}

/// A Feature of the Software Plant, or one that lists the tag: its slug, whether it lists the
/// tag, and the names of the external Components it mentions.
struct Simulator {
    slug: String,
    tagged: bool,
    mentioned: BTreeSet<String>,
}

/// The external Components of the document, by key, with their names.
fn external_components(document: &Document) -> BTreeMap<ComponentId, String> {
    let boundaries = Boundaries::of(document);
    let mut external = BTreeMap::new();
    document.walk(&mut |node| {
        if let Node::Component(component) = node
            && let Some(id) = component.id
            && boundaries.is_external(id)
        {
            external.insert(id, component.name.clone());
        }
    });
    external
}

fn simulators(document: &Document) -> Vec<Simulator> {
    let external = external_components(document);
    let mentions = |feature: &Feature| -> BTreeSet<String> {
        let source = feature.id.map(|id| ElementId::Node(NodeId::Feature(id)));
        document
            .edges
            .iter()
            .filter_map(|edge| match edge {
                Edge::Mentions {
                    source: from,
                    target: ElementId::Node(NodeId::Component(target)),
                } if Some(*from) == source => external.get(target).cloned(),
                _ => None,
            })
            .collect()
    };
    let mut found = Vec::new();
    for product in &document.products {
        let plant = product.slug.as_str() == PLANT;
        for feature in product
            .feature_groups
            .iter()
            .flat_map(|group| &group.features)
        {
            let tagged = feature.details.tags.iter().any(|name| name.as_str() == TAG);
            if plant || tagged {
                found.push(Simulator {
                    slug: feature.slug.as_str().to_owned(),
                    tagged,
                    mentioned: mentions(feature),
                });
            }
        }
    }
    found
}

fn tagged(document: &Document) -> Vec<Simulator> {
    simulators(document)
        .into_iter()
        .filter(|simulator| simulator.tagged)
        .collect()
}

#[test]
fn each_simulator_mentions_exactly_one_external_component() {
    let found = tagged(&document());
    assert!(!found.is_empty(), "no Feature lists `{TAG}`");
    for simulator in found {
        let Simulator {
            slug, mentioned, ..
        } = simulator;
        assert_eq!(mentioned.len(), 1, "{slug} mentions {mentioned:?}");
    }
}

#[test]
fn each_simulator_has_a_contract_suite_and_each_suite_a_simulator() {
    let tagged: BTreeSet<String> = tagged(&document())
        .into_iter()
        .map(|simulator| simulator.slug)
        .collect();
    let listed: BTreeSet<String> = SUITES.iter().map(|(slug, _)| (*slug).to_owned()).collect();
    assert_eq!(
        tagged, listed,
        "the Features that list `{TAG}` and the suites"
    );
    for (slug, suite) in SUITES {
        let file = repository().join(suite);
        assert!(file.is_file(), "{slug}: no suite at {}", file.display());
    }
}

#[test]
fn each_feature_of_the_plant_that_stands_in_for_an_external_system_lists_the_tag() {
    for simulator in simulators(&document()) {
        assert!(
            simulator.tagged || simulator.mentioned.is_empty(),
            "{} mentions {:?} but does not list `{TAG}`",
            simulator.slug,
            simulator.mentioned
        );
    }
}

#[test]
fn only_the_repository_port_runs_git() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for entry in std::fs::read_dir(&src).unwrap() {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        let runs = text.contains("Command::new(") || text.contains("process::Command");
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        assert!(!runs || name == "repository.rs", "{name} runs a command");
    }
}
