//! The root, `yardlog.yaml`: the format, Yard Log's key, branch, title and revision, the Act that
//! opened it, its stages in order by key with each one's id, its submissions, the keys of Yard
//! Logs merged into it, its close, and what is left to settle after a merge.

use loftspec::yaml::{Node, Out};

use super::acts::{Field, read_act, read_arrival, text, token, write_act};
use super::reader::{Fields, Read, at, scalar};
use crate::format::Format;
use crate::keys::{BlockKey, StageKey, SubmissionKey, YardLogKey};
use crate::log::{Change, Happened, Root, Settling, StageEntry, Submission};

pub(super) fn write(root: &Root) -> Out {
    let mut fields: Vec<Field> = vec![
        ("format", token(&root.format)),
        ("key", token(&root.key)),
        ("branch", text(&root.branch)),
    ];
    if let Some(name) = &root.opened_as {
        fields.push(("openedAs", text(name)));
    }
    fields.push(("title", text(&root.title)));
    fields.push(("revision", Out::Integer(root.revision)));
    fields.push(("opened", Out::Mapping(write_act(&root.opened, None))));
    let mut list = |name, items: Vec<Out>| {
        if !items.is_empty() {
            fields.push((name, Out::Sequence(items)));
        }
    };
    list("stages", root.stages.iter().map(stage).collect());
    list(
        "submissions",
        root.submissions.iter().map(submission).collect(),
    );
    list("merged", root.merged.iter().map(token).collect());
    if let Some(closed) = &root.closed {
        fields.push(("closed", Out::Mapping(write_act(closed, None))));
    }
    if let Some(since) = root.unsettled_since {
        fields.push(("unsettledSince", Out::Integer(since)));
    }
    if !root.settlings.is_empty() {
        let settlings = root.settlings.iter().map(settling).collect();
        fields.push(("settlings", Out::Sequence(settlings)));
    }
    Out::Mapping(fields)
}

fn stage(entry: &StageEntry) -> Out {
    Out::Mapping(vec![("key", token(&entry.key)), ("id", token(&entry.id))])
}

fn submission(submission: &Submission) -> Out {
    let blocks = submission.blocks.iter().map(token).collect();
    let changes = submission
        .changes
        .iter()
        .map(|change| {
            let mut fields = vec![("what", token(&change.what.as_str()))];
            fields.extend(write_act(&change.act, change.arrived));
            Out::Mapping(fields)
        })
        .collect();
    Out::Mapping(vec![
        ("key", token(&submission.key)),
        ("blocks", Out::Sequence(blocks)),
        ("changes", Out::Sequence(changes)),
    ])
}

fn settling(settling: &Settling) -> Out {
    let mut fields = write_act(&settling.act, None);
    let renamed: Vec<Out> = settling
        .renamed
        .iter()
        .map(|(key, from, to)| {
            Out::Mapping(vec![
                ("block", token(key)),
                ("from", token(&from)),
                ("to", token(&to)),
            ])
        })
        .collect();
    let renumbered: Vec<Out> = settling
        .renumbered
        .iter()
        .map(|(key, from, to)| {
            Out::Mapping(vec![
                ("block", token(key)),
                ("from", Out::Integer(u64::from(*from))),
                ("to", Out::Integer(u64::from(*to))),
            ])
        })
        .collect();
    if !renamed.is_empty() {
        fields.push(("renamed", Out::Sequence(renamed)));
    }
    if !renumbered.is_empty() {
        fields.push(("renumbered", Out::Sequence(renumbered)));
    }
    Out::Mapping(fields)
}

/// The format the root names, read before the rest so that a newer one is told apart.
pub(super) fn format(node: &Node) -> Read<Format> {
    let mut fields = Fields::of(node)?;
    let found = fields.required("format")?;
    let text = scalar(found)?;
    Format::parse(&text)
        .ok_or_else(|| at(found.position, format!("`{text}` is not a yardlog format")))
}

pub(super) fn read(node: &Node) -> Read<Root> {
    let mut fields = Fields::of(node)?;
    let format = fields.word("format", Format::parse)?;
    let key = YardLogKey::new(fields.key("key")?);
    let branch = fields.text("branch")?;
    let opened_as = fields.optional_text("openedAs")?;
    let title = fields.text("title")?;
    let revision = fields.integer("revision")?;
    let opened = read_act(&mut Fields::of(fields.required("opened")?)?)?;
    let mut stages = Vec::new();
    for node in fields.list("stages")? {
        let mut entry = Fields::of(node)?;
        stages.push(StageEntry {
            key: StageKey::new(entry.key("key")?),
            id: entry.handle("id")?,
        });
        entry.finish()?;
    }
    let mut submissions = Vec::new();
    for node in fields.list("submissions")? {
        submissions.push(read_submission(node)?);
    }
    let mut merged = Vec::new();
    for node in fields.list("merged")? {
        merged.push(YardLogKey::new(super::reader::key(node)?));
    }
    let closed = match fields.get("closed") {
        Some(node) => Some(read_act(&mut Fields::of(node)?)?),
        None => None,
    };
    let unsettled_since = fields.optional_integer("unsettledSince")?;
    let mut settlings = Vec::new();
    for node in fields.list("settlings")? {
        settlings.push(read_settling(node)?);
    }
    fields.finish()?;
    Ok(Root {
        format,
        key,
        branch,
        opened_as,
        title,
        revision,
        opened,
        stages,
        submissions,
        merged,
        closed,
        unsettled_since,
        settlings,
    })
}

fn read_submission(node: &Node) -> Read<Submission> {
    let mut fields = Fields::of(node)?;
    let key = SubmissionKey::new(fields.key("key")?);
    let mut blocks = Vec::new();
    for node in fields.list("blocks")? {
        blocks.push(BlockKey::new(super::reader::key(node)?));
    }
    let mut changes = Vec::new();
    for node in fields.list("changes")? {
        let mut change = Fields::of(node)?;
        let what = change.word("what", Happened::parse)?;
        changes.push(Change {
            what,
            act: read_act(&mut change)?,
            arrived: read_arrival(&mut change)?,
        });
        change.finish()?;
    }
    if !changes.iter().any(|change| change.what == Happened::Sent) {
        return Err(at(node.position, "a submission records its send"));
    }
    fields.finish()?;
    Ok(Submission {
        key,
        blocks,
        changes,
    })
}

fn read_settling(node: &Node) -> Read<Settling> {
    let mut fields = Fields::of(node)?;
    let act = read_act(&mut fields)?;
    let mut renamed = Vec::new();
    for node in fields.list("renamed")? {
        let mut entry = Fields::of(node)?;
        renamed.push((
            BlockKey::new(entry.key("block")?),
            entry.handle("from")?,
            entry.handle("to")?,
        ));
        entry.finish()?;
    }
    let mut renumbered = Vec::new();
    for node in fields.list("renumbered")? {
        let mut entry = Fields::of(node)?;
        let number = |entry: &mut Fields<'_>, name| {
            entry.integer(name).and_then(|n| {
                u32::try_from(n).map_err(|_| at(node.position, "the number is too large"))
            })
        };
        renumbered.push((
            BlockKey::new(entry.key("block")?),
            number(&mut entry, "from")?,
            number(&mut entry, "to")?,
        ));
        entry.finish()?;
    }
    fields.finish()?;
    Ok(Settling {
        act,
        renamed,
        renumbered,
    })
}
