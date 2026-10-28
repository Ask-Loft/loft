//! Acts and arrival marks in the files: `at`, `revision` and `by` among a record's fields, and
//! `arrived` where a merge brought the record.

use loft_common::yaml::Out;

use super::reader::{Fields, Read, at};
use crate::act::{Act, Actor, Agent, Subagent};
use crate::log::Arrival;

pub(super) type Field = (&'static str, Out);

pub(super) fn text(value: &str) -> Out {
    Out::Text(value.to_owned())
}

pub(super) fn token<T: std::fmt::Display + ?Sized>(value: &T) -> Out {
    Out::Token(value.to_string())
}

/// The fields of an Act, then the arrival mark, as a record lists them.
pub(super) fn write_act(act: &Act, arrived: Option<Arrival>) -> Vec<Field> {
    let mut fields = vec![
        ("at", token(&act.at)),
        ("revision", Out::Integer(act.revision)),
        ("by", write_actor(&act.actor)),
    ];
    match arrived {
        None => {}
        Some(Arrival::Pending) => fields.push(("arrived", token("pending"))),
        Some(Arrival::At(revision)) => fields.push(("arrived", Out::Integer(revision))),
    }
    fields
}

pub(super) fn write_actor(actor: &Actor) -> Out {
    match actor {
        Actor::Agent(agent) => {
            let mut fields = vec![
                ("host", token(&agent.host)),
                ("session", text(&agent.session)),
            ];
            if let Some(sub) = &agent.subagent {
                fields.push(("agent", text(&sub.id)));
                fields.push(("agentType", text(&sub.kind)));
            }
            Out::Mapping(fields)
        }
        Actor::Person { name, email } => {
            Out::Mapping(vec![("name", text(name)), ("email", text(email))])
        }
    }
}

pub(super) fn read_act(fields: &mut Fields<'_>) -> Read<Act> {
    let at_time = fields.time("at")?;
    let revision = fields.integer("revision")?;
    let actor = read_actor(fields.required("by")?)?;
    Ok(Act {
        at: at_time,
        revision,
        actor,
    })
}

pub(super) fn read_arrival(fields: &mut Fields<'_>) -> Read<Option<Arrival>> {
    let Some(node) = fields.get("arrived") else {
        return Ok(None);
    };
    let text = super::reader::scalar(node)?;
    if text == "pending" {
        return Ok(Some(Arrival::Pending));
    }
    text.parse()
        .map(|revision| Some(Arrival::At(revision)))
        .map_err(|_| at(node.position, "`arrived` is `pending` or a revision"))
}

pub(super) fn read_actor(node: &loft_common::yaml::Node) -> Read<Actor> {
    let mut fields = Fields::of(node)?;
    let actor = if fields.get("name").is_some() {
        Actor::Person {
            name: fields.text("name")?,
            email: fields.text("email")?,
        }
    } else {
        let host = fields.text("host")?;
        let session = fields.text("session")?;
        let subagent = match fields.optional_text("agent")? {
            Some(id) => Some(Subagent {
                id,
                kind: fields.text("agentType")?,
            }),
            None => None,
        };
        Actor::Agent(Agent {
            host,
            session,
            subagent,
        })
    };
    fields.finish()?;
    Ok(actor)
}
