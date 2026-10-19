//! `loft review` (the `loft review` Feature): the review packet of the tree, against the tree
//! where HEAD left a base revision where `--base` is given, narrowed to a node by `--node`, and
//! paged by whole units. The packet is JSON; the text is an index of the page. It writes
//! nothing and judges nothing (the decision "The review packet carries context and no
//! judgement").

use std::path::Path;

use loftspec::check::hashed;
use loftspec::codec::{self, Sites};
use loftspec::document::Document;
use loftspec::ids::NodeId;
use loftspec::review::{Assertion, Change, Fact, Holder, Linked, Packet, Unit, WithCriteria, pack};
use loftspec_store::repository::GitCommand;
use serde_json::{Map, Value, json};

use crate::finish::Finished;
use crate::json;
use crate::out::outln;
use crate::stop::{self, Stop};
use crate::{base, version};

/// The schema `--format json` writes to, by name and version.
const SCHEMA: &str = "loft-review/1";

/// What `loft review` was asked for.
pub struct Options<'a> {
    pub base: Option<&'a str>,
    pub node: Option<&'a str>,
    pub max_chars: usize,
    pub page: usize,
}

/// The page of the packet asked for, with where it came from.
struct Paged {
    provenance: Value,
    units: Vec<(Value, usize)>,
    number: usize,
    of: usize,
}

pub fn run(root: &Path, options: &Options, as_json: bool) -> Finished {
    let paged = match attempt(root, options) {
        Ok(paged) => paged,
        Err(stop) => return stop.exit(SCHEMA, as_json),
    };
    let outcome = if paged.units.is_empty() {
        "empty"
    } else {
        "packed"
    };
    if as_json {
        let units: Vec<Value> = paged.units.into_iter().map(|(unit, _)| unit).collect();
        let fields = json!({
            "provenance": paged.provenance,
            "page": { "number": paged.number, "of": paged.of, "maxChars": options.max_chars },
            "units": units,
        });
        json::print(&json::result(SCHEMA, outcome, fields, Vec::new()));
    } else {
        for (unit, size) in &paged.units {
            outln!("{}", index_line(unit, *size));
        }
        outln!("page {} of {}", paged.number, paged.of);
    }
    Finished::success(outcome)
}

fn attempt(root: &Path, options: &Options) -> Result<Paged, Stop> {
    let files = stop::read_files(root)?;
    let (document, sites) = codec::read_sited(&files)?;
    let state_root = hashed(&document, &sites)
        .map_err(|errors| Stop::invalid(&errors))?
        .roots
        .state;
    let node = options
        .node
        .map(|query| named(&document, query))
        .transpose()?;
    let base = match options.base {
        Some(revision) => Some(read_base(root, revision)?),
        None => None,
    };
    let packet = pack(
        &document,
        base.as_ref().map(|(_, document, _)| document),
        node,
    )
    .expect("a tree with every key packs");
    let render = Render {
        sites: &sites,
        base_sites: base.as_ref().map(|(_, _, sites)| sites),
    };
    let provenance = json!({
        "loft": version::VERSION,
        "stateRoot": state_root.to_string(),
        "base": base.as_ref().map(|((revision, commit), ..)| {
            json!({ "revision": revision, "commit": commit })
        }),
        "node": options.node,
    });
    page(render.packet(&packet), options, provenance)
}

/// The base revision's name and commit, and its tree.
type Base = ((String, String), Document, Sites);

fn read_base(root: &Path, revision: &str) -> Result<Base, Stop> {
    let base = base::read(&GitCommand::default(), root, revision)?;
    let (document, sites) =
        codec::read_sited(&base.files).map_err(|errors| base::unread(&base.commit, &errors))?;
    Ok(((revision.to_owned(), base.commit), document, sites))
}

/// The one node `query` names by its key, its slug or its label, the label in any case.
fn named(document: &Document, query: &str) -> Result<NodeId, Stop> {
    let mut found = Vec::new();
    document.walk(&mut |node| {
        let Some(id) = node.id() else { return };
        let label = document.bindings.label_of(id);
        let identified = id.key().to_string() == query
            || node.slug().is_some_and(|slug| slug.as_str() == query)
            || label.is_some_and(|label| label.to_string().eq_ignore_ascii_case(query));
        if identified {
            found.push(id);
        }
    });
    match found.as_slice() {
        [id] => Ok(*id),
        [] => Err(Stop::one("node", format!("`{query}` names no node"))),
        many => {
            let keys: Vec<String> = many.iter().map(|id| id.key().to_string()).collect();
            let message = format!("`{query}` names more than one node: {}", keys.join(", "));
            Err(Stop::one("node", message))
        }
    }
}

/// The page `options` asks for: whole units, each page taking every next unit that fits, the
/// units moved and none copied.
fn page(units: Vec<Value>, options: &Options, provenance: Value) -> Result<Paged, Stop> {
    let mut pages: Vec<Vec<(Value, usize)>> = Vec::new();
    let mut taken = 0;
    for mut unit in units {
        let size = serde_json::to_string(&unit)
            .expect("a unit serializes")
            .chars()
            .count();
        if pages.is_empty() || taken + size > options.max_chars {
            pages.push(Vec::new());
            taken = 0;
        }
        if size > options.max_chars {
            unit["oversized"] = Value::Bool(true);
        }
        pages
            .last_mut()
            .expect("a page was opened")
            .push((unit, size));
        taken += size;
    }
    let of = pages.len().max(1);
    if options.page > of {
        let pages = if of == 1 { "page" } else { "pages" };
        let message = format!(
            "page {} is past the last page; the packet has {of} {pages}",
            options.page
        );
        return Err(Stop::one("page", message));
    }
    let units = pages.into_iter().nth(options.page - 1).unwrap_or_default();
    Ok(Paged {
        provenance,
        units,
        number: options.page,
        of,
    })
}

/// A unit as one line of the index: its scope's label and name, its number of assertions and
/// its number of characters.
fn index_line(unit: &Value, size: usize) -> String {
    let scope = &unit["scope"]["node"];
    let label = scope["label"].as_str().unwrap_or("-");
    let name = scope["name"].as_str().unwrap_or_default();
    let count = unit["assertions"].as_array().map_or(0, Vec::len);
    format!("{label}  {name}  {count} assertions  {size} characters")
}

/// The JSON of a packet, each node at its site in the tree or, where only the base holds it, in
/// the base.
struct Render<'a> {
    sites: &'a Sites,
    base_sites: Option<&'a Sites>,
}

impl Render<'_> {
    fn packet(&self, packet: &Packet) -> Vec<Value> {
        packet.units.iter().map(|unit| self.unit(unit)).collect()
    }

    fn unit(&self, unit: &Unit) -> Value {
        let holders = |holders: &[Holder]| -> Vec<Value> {
            holders.iter().map(|holder| self.holder(holder)).collect()
        };
        let requirements: Vec<Value> = unit
            .requirements
            .iter()
            .map(|WithCriteria { requirement, criteria }| {
                json!({ "requirement": self.fact(requirement), "criteria": self.facts(criteria) })
            })
            .collect();
        let realizes: Vec<Value> = unit
            .realizes
            .iter()
            .map(|realized| {
                json!({
                    "component": self.holder(&realized.component),
                    "above": holders(&realized.above),
                })
            })
            .collect();
        let terms: Vec<Value> = unit
            .terms
            .iter()
            .map(|term| {
                json!({
                    "key": term.id.key().to_string(),
                    "name": term.name,
                    "definition": term.definition,
                    "aliases": term.aliases,
                })
            })
            .collect();
        json!({
            "scope": self.holder(&unit.scope),
            "ancestors": holders(&unit.ancestors),
            "requirements": requirements,
            "realizes": realizes,
            "assertions": unit.assertions.iter().map(|a| self.assertion(a)).collect::<Vec<_>>(),
            "decisions": unit.decisions.iter().map(decision).collect::<Vec<_>>(),
            "terms": terms,
        })
    }

    fn holder(&self, holder: &Holder) -> Value {
        json!({
            "node": self.fact(&holder.node),
            "constraints": self.facts(&holder.constraints),
            "qualityAttributes": self.facts(&holder.quality_attributes),
        })
    }

    fn facts(&self, facts: &[Fact]) -> Vec<Value> {
        facts.iter().map(|fact| self.fact(fact)).collect()
    }

    fn fact(&self, fact: &Fact) -> Value {
        let sites = match (fact.from_base, self.base_sites) {
            (true, Some(base)) => base,
            _ => self.sites,
        };
        let key = fact.id.key();
        let mut fields = Map::new();
        fields.insert("key".to_owned(), json!(key.to_string()));
        fields.insert("label".to_owned(), json!(fact.label.map(|l| l.to_string())));
        fields.insert("kind".to_owned(), json!(fact.kind().as_str()));
        if let Some(site) = sites.get(key) {
            fields.extend(json::site(site));
        }
        fields.insert("name".to_owned(), json!(fact.name));
        fields.insert("view".to_owned(), fact.view.clone());
        fields.insert("change".to_owned(), json!(fact.change.map(Change::as_str)));
        Value::Object(fields)
    }

    fn assertion(&self, assertion: &Assertion) -> Value {
        json!({
            "key": assertion.id.key().to_string(),
            "kind": assertion.kind().as_str(),
            "change": assertion.change.map(Change::as_str),
            "before": assertion.before.as_ref().map(|before| self.fact(before)),
        })
    }
}

fn decision(linked: &Linked) -> Value {
    let links: Vec<Value> = linked
        .links
        .iter()
        .map(|(node, via)| json!({ "node": node.key().to_string(), "via": via.as_str() }))
        .collect();
    json!({
        "key": linked.id.key().to_string(),
        "label": linked.label.map(|label| label.to_string()),
        "title": linked.title,
        "state": linked.state.as_str(),
        "outcome": linked.outcome,
        "links": links,
    })
}
