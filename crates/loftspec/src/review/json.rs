//! The JSON of a review packet's units, each node at its site in the tree or, where only the base
//! holds it, in the base. Every caller writes a unit from these fields alone, so the same packet
//! gives the same bytes (The review packet).

use serde_json::{Value, json};

use super::{Assertion, Change, Fact, Holder, Linked, Packet, Unit, WithCriteria};
use crate::codec::Sites;
use crate::query::element_fields;

pub(super) struct Render<'a> {
    pub sites: &'a Sites,
    pub base_sites: Option<&'a Sites>,
}

impl Render<'_> {
    pub(super) fn packet(&self, packet: &Packet) -> Vec<Value> {
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
        let kind = fact.kind().as_str();
        let mut fields = element_fields(key, fact.label, kind, sites.get(key), &fact.name);
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
