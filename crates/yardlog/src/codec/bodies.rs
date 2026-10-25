//! The fields of one version of a block, for each kind.

use loft_common::yaml::Out;

use super::acts::{Field, text, token};
use super::reader::{Fields, Read, at};
use crate::body::{
    Approval, Body, Choice, Decision, Kind, Loft, Note, Plan, Question, Reason, Recorded, Stage,
    Status, Task, TaskState, TaskType, Tone, View,
};
use crate::datum::Datum;
use crate::keys::BlockKey;

fn optional(fields: &mut Vec<Field>, name: &'static str, value: Option<&String>) {
    if let Some(value) = value {
        fields.push((name, text(value)));
    }
}

fn flag(fields: &mut Vec<Field>, name: &'static str, value: bool) {
    if value {
        fields.push((name, Out::Plain("true".to_owned())));
    }
}

pub(super) fn write_body(body: &Body) -> Vec<Field> {
    let mut fields = Vec::new();
    match body {
        Body::Stage(stage) => {
            fields.push(("title", text(&stage.title)));
            optional(&mut fields, "goal", stage.goal.as_ref());
            fields.push(("status", token(&stage.status.as_str())));
        }
        Body::Note(note) => {
            optional(&mut fields, "title", note.title.as_ref());
            fields.push(("markdown", text(&note.markdown)));
            if let Some(tone) = note.tone {
                fields.push(("tone", token(&tone.as_str())));
            }
            flag(&mut fields, "folded", note.folded);
        }
        Body::Question(question) => write_question(&mut fields, question),
        Body::Approval(approval) => {
            fields.push(("title", text(&approval.title)));
            fields.push(("markdown", text(&approval.markdown)));
        }
        Body::Decision(decision) => {
            fields.push(("title", text(&decision.title)));
            fields.push(("consequence", text(&decision.consequence)));
            if let Some(question) = decision.question {
                fields.push(("question", token(&question)));
            }
        }
        Body::Task(task) => {
            fields.push(("title", text(&task.title)));
            optional(&mut fields, "detail", task.detail.as_ref());
            fields.push(("state", token(&task.state.as_str())));
            if task.kind != TaskType::Regular {
                fields.push(("type", token(&task.kind.as_str())));
            }
        }
        Body::Plan(plan) => {
            fields.push(("title", text(&plan.title)));
            fields.push(("markdown", text(&plan.markdown)));
            optional(&mut fields, "path", plan.path.as_ref());
            flag(&mut fields, "planMode", plan.plan_mode);
        }
        Body::Loft(loft) => write_loft(&mut fields, loft),
    }
    fields
}

fn write_question(fields: &mut Vec<Field>, question: &Question) {
    fields.push(("prompt", text(&question.prompt)));
    fields.push(("header", text(&question.header)));
    optional(fields, "context", question.context.as_ref());
    let options = question
        .options
        .iter()
        .map(|choice| {
            let mut option = vec![("label", text(&choice.label))];
            optional(&mut option, "detail", choice.detail.as_ref());
            flag(&mut option, "recommended", choice.recommended);
            Out::Mapping(option)
        })
        .collect();
    fields.push(("options", Out::Sequence(options)));
    flag(fields, "multiple", question.multiple);
    flag(fields, "ownWords", question.own_words);
}

fn write_loft(fields: &mut Vec<Field>, loft: &Loft) {
    fields.push(("view", token(&loft.view.as_str())));
    optional(fields, "note", loft.note.as_ref());
    optional(fields, "base", loft.base.as_ref());
    optional(fields, "decision", loft.decision.as_ref());
    if !loft.reasons.is_empty() {
        let reasons = loft
            .reasons
            .iter()
            .map(|reason| {
                Out::Mapping(vec![
                    ("rule", token(&reason.rule)),
                    ("key", token(&reason.key)),
                    ("reason", text(&reason.reason)),
                ])
            })
            .collect();
        fields.push(("reasons", Out::Sequence(reasons)));
    }
    let recorded = &loft.recorded;
    let mut place = Vec::new();
    optional(&mut place, "commit", recorded.commit.as_ref());
    optional(&mut place, "branch", recorded.branch.as_ref());
    flag(&mut place, "dirty", recorded.dirty);
    place.push(("loft", token(&recorded.loft)));
    place.push(("at", token(&recorded.at)));
    fields.push(("recorded", Out::Mapping(place)));
    let json = serde_json::Value::from(&loft.report);
    let pretty = serde_json::to_string_pretty(&json).expect("a JSON value always writes");
    fields.push(("report", Out::Text(format!("{pretty}\n"))));
}

pub(super) fn read_body(kind: Kind, fields: &mut Fields<'_>) -> Read<Body> {
    Ok(match kind {
        Kind::Stage => Body::Stage(Stage {
            title: fields.text("title")?,
            goal: fields.optional_text("goal")?,
            status: fields.word("status", Status::parse)?,
        }),
        Kind::Note => Body::Note(Note {
            title: fields.optional_text("title")?,
            markdown: fields.text("markdown")?,
            tone: fields.optional_word("tone", Tone::parse)?,
            folded: fields.flag("folded")?,
        }),
        Kind::Question => Body::Question(read_question(fields)?),
        Kind::Approval => Body::Approval(Approval {
            title: fields.text("title")?,
            markdown: fields.text("markdown")?,
        }),
        Kind::Decision => Body::Decision(Decision {
            title: fields.text("title")?,
            consequence: fields.text("consequence")?,
            question: match fields.get("question") {
                Some(node) => Some(BlockKey::new(super::reader::key(node)?)),
                None => None,
            },
        }),
        Kind::Task => Body::Task(Task {
            title: fields.text("title")?,
            detail: fields.optional_text("detail")?,
            state: fields.word("state", TaskState::parse)?,
            kind: fields
                .optional_word("type", TaskType::parse)?
                .unwrap_or(TaskType::Regular),
        }),
        Kind::Plan => Body::Plan(Plan {
            title: fields.text("title")?,
            markdown: fields.text("markdown")?,
            path: fields.optional_text("path")?,
            plan_mode: fields.flag("planMode")?,
        }),
        Kind::Loft => Body::Loft(Box::new(read_loft(fields)?)),
    })
}

fn read_question(fields: &mut Fields<'_>) -> Read<Question> {
    let prompt = fields.text("prompt")?;
    let header = fields.text("header")?;
    let context = fields.optional_text("context")?;
    let mut options = Vec::new();
    for node in fields.list("options")? {
        let mut option = Fields::of(node)?;
        options.push(Choice {
            label: option.text("label")?,
            detail: option.optional_text("detail")?,
            recommended: option.flag("recommended")?,
        });
        option.finish()?;
    }
    Ok(Question {
        prompt,
        header,
        context,
        options,
        multiple: fields.flag("multiple")?,
        own_words: fields.flag("ownWords")?,
    })
}

fn read_loft(fields: &mut Fields<'_>) -> Read<Loft> {
    let view = fields.word("view", View::parse)?;
    let note = fields.optional_text("note")?;
    let base = fields.optional_text("base")?;
    let decision = fields.optional_text("decision")?;
    let mut reasons = Vec::new();
    for node in fields.list("reasons")? {
        let mut entry = Fields::of(node)?;
        reasons.push(Reason {
            rule: entry.text("rule")?,
            key: entry.key("key")?,
            reason: entry.text("reason")?,
        });
        entry.finish()?;
    }
    let mut place = Fields::of(fields.required("recorded")?)?;
    let recorded = Recorded {
        commit: place.optional_text("commit")?,
        branch: place.optional_text("branch")?,
        dirty: place.flag("dirty")?,
        loft: place.text("loft")?,
        at: place.time("at")?,
    };
    place.finish()?;
    let written = fields.required("report")?;
    let json: serde_json::Value = serde_json::from_str(&super::reader::scalar(written)?)
        .map_err(|error| at(written.position, format!("the report is not JSON: {error}")))?;
    Ok(Loft {
        view,
        note,
        base,
        decision,
        reasons,
        recorded,
        report: Datum::from(&json),
    })
}
