//! A stage's file, `<stage key>.yaml`: the stage's key, id and versions, then its blocks in
//! order, each with its versions and the person's answers or outcomes.

use loft_common::yaml::{Node, Out};

use super::acts::{Field, read_act, read_arrival, text, token, write_act};
use super::bodies::{read_body, write_body};
use super::reader::{Fields, Read, at, each};
use crate::act::Channel;
use crate::body::{Kind, Verdict};
use crate::keys::{BlockKey, ResponseKey, VersionKey};
use crate::log::{Block, Given, Response, StageFile, Version};

pub(super) fn write(file: &StageFile) -> Out {
    let mut fields = vec![
        ("key", token(&file.stage.key)),
        ("id", token(&file.stage.id)),
        ("versions", versions(&file.stage)),
    ];
    if !file.blocks.is_empty() {
        let blocks = file.blocks.iter().map(write_block).collect();
        fields.push(("blocks", Out::Sequence(blocks)));
    }
    Out::Mapping(fields)
}

fn write_block(block: &Block) -> Out {
    let mut fields = vec![
        ("key", token(&block.key)),
        ("id", token(&block.id)),
        ("kind", token(&block.kind.as_str())),
    ];
    if let Some(number) = block.number {
        fields.push(("number", Out::Integer(u64::from(number))));
    }
    fields.push(("versions", versions(block)));
    if !block.responses.is_empty() {
        let responses = block.responses.iter().map(write_response).collect();
        fields.push(("responses", Out::Sequence(responses)));
    }
    Out::Mapping(fields)
}

fn versions(block: &Block) -> Out {
    Out::Sequence(
        block
            .versions
            .iter()
            .map(|version| {
                let mut fields: Vec<Field> = vec![("key", token(&version.key))];
                fields.extend(write_act(&version.act, version.arrived));
                fields.extend(write_body(&version.body));
                Out::Mapping(fields)
            })
            .collect(),
    )
}

fn write_response(response: &Response) -> Out {
    let mut fields: Vec<Field> = vec![("key", token(&response.key))];
    fields.extend(write_act(&response.act, response.arrived));
    fields.push(("channel", token(&response.channel.as_str())));
    match &response.given {
        Given::Answer {
            chosen,
            words,
            notes,
        } => {
            if !chosen.is_empty() {
                let labels = chosen.iter().map(|label| text(label)).collect();
                fields.push(("chosen", Out::Sequence(labels)));
            }
            if let Some(words) = words {
                fields.push(("words", text(words)));
            }
            if let Some(notes) = notes {
                fields.push(("notes", text(notes)));
            }
        }
        Given::Outcome { verdict, reason } => {
            fields.push(("verdict", token(&verdict.as_str())));
            if let Some(reason) = reason {
                fields.push(("reason", text(reason)));
            }
        }
    }
    Out::Mapping(fields)
}

pub(super) fn read(node: &Node) -> Read<StageFile> {
    let mut fields = Fields::of(node)?;
    let key = BlockKey::new(fields.key("key")?);
    let id = fields.handle("id")?;
    let versions = read_versions(Kind::Stage, &mut fields)?;
    let blocks = each(fields.list("blocks")?, read_block)?;
    fields.finish()?;
    Ok(StageFile {
        stage: Block {
            key,
            id,
            kind: Kind::Stage,
            number: None,
            versions,
            responses: Vec::new(),
        },
        blocks,
    })
}

fn read_block(node: &Node) -> Read<Block> {
    let mut fields = Fields::of(node)?;
    let key = BlockKey::new(fields.key("key")?);
    let id = fields.handle("id")?;
    let kind = fields.word("kind", Kind::parse)?;
    if kind == Kind::Stage {
        return Err(at(node.position, "a stage is not a block of another stage"));
    }
    let number = fields
        .optional_integer("number")?
        .map(|number| {
            u32::try_from(number).map_err(|_| at(node.position, "the number is too large"))
        })
        .transpose()?;
    let versions = read_versions(kind, &mut fields)?;
    let mut responses = Vec::new();
    for node in fields.list("responses")? {
        responses.push(read_response(kind, node)?);
    }
    fields.finish()?;
    Ok(Block {
        key,
        id,
        kind,
        number,
        versions,
        responses,
    })
}

fn read_versions(kind: Kind, fields: &mut Fields<'_>) -> Read<Vec<Version>> {
    let position = fields.position;
    let mut versions = Vec::new();
    for node in fields.list("versions")? {
        let mut version = Fields::of(node)?;
        versions.push(Version {
            key: VersionKey::new(version.key("key")?),
            act: read_act(&mut version)?,
            arrived: read_arrival(&mut version)?,
            body: read_body(kind, &mut version)?,
        });
        version.finish()?;
    }
    if versions.is_empty() {
        return Err(at(position, "a block holds at least one version"));
    }
    Ok(versions)
}

fn read_response(kind: Kind, node: &Node) -> Read<Response> {
    let mut fields = Fields::of(node)?;
    let key = ResponseKey::new(fields.key("key")?);
    let act = read_act(&mut fields)?;
    let arrived = read_arrival(&mut fields)?;
    let channel = fields.word("channel", Channel::parse)?;
    let given = if kind == Kind::Question {
        let mut chosen = Vec::new();
        for label in fields.list("chosen")? {
            chosen.push(super::reader::scalar(label)?);
        }
        Given::Answer {
            chosen,
            words: fields.optional_text("words")?,
            notes: fields.optional_text("notes")?,
        }
    } else {
        Given::Outcome {
            verdict: fields.word("verdict", Verdict::parse)?,
            reason: fields.optional_text("reason")?,
        }
    };
    fields.finish()?;
    Ok(Response {
        key,
        act,
        channel,
        arrived,
        given,
    })
}
