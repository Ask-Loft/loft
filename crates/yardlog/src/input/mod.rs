//! A batch as an agent writes it: a JSON list of blocks, each with its id, its kind and its
//! kind's fields. Reading a batch checks each block against its kind alone; the rules that
//! need Yard Log are applied when the batch is written.

mod fields;

use serde_json::Value;

use crate::body::{
    Approval, Choice, Kind, Note, Plan, Question, Reason, Stage, Status, Task, TaskState, TaskType,
    Tone, View,
};
use crate::keys::Handle;

pub use fields::Fault;
use fields::Fields;

/// One block of a batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Input {
    pub id: Handle,
    /// The stage that holds it; none for a stage.
    pub stage: Option<Handle>,
    /// The block or stage a new one is placed after.
    pub after: Option<Handle>,
    pub request: Request,
}

/// What a block of a batch asks to be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    Stage(Stage),
    Note(Note),
    Question(Question),
    Approval(Approval),
    Decision {
        title: String,
        consequence: String,
        /// The question it settles, by its id.
        question: Option<Handle>,
    },
    Task(Task),
    Plan(Plan),
    Loft(LoftRequest),
}

impl Request {
    #[must_use]
    pub const fn kind(&self) -> Kind {
        match self {
            Self::Stage(_) => Kind::Stage,
            Self::Note(_) => Kind::Note,
            Self::Question(_) => Kind::Question,
            Self::Approval(_) => Kind::Approval,
            Self::Decision { .. } => Kind::Decision,
            Self::Task(_) => Kind::Task,
            Self::Plan(_) => Kind::Plan,
            Self::Loft(_) => Kind::Loft,
        }
    }
}

/// The `loft` view an agent asks to be recorded; `loft` computes its report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoftRequest {
    pub view: View,
    pub note: Option<String>,
    pub base: Option<String>,
    pub decision: Option<String>,
    pub reasons: Vec<Reason>,
}

/// Reads a batch.
///
/// # Errors
///
/// Every fault of every block, by the block's id, or its place where it has no usable id.
pub fn read_batch(batch: &Value) -> Result<Vec<Input>, Vec<Fault>> {
    let Value::Array(items) = batch else {
        return Err(vec![Fault {
            block: String::new(),
            field: String::new(),
            message: "a batch is a list of blocks".to_owned(),
        }]);
    };
    let mut inputs = Vec::new();
    let mut faults = Vec::new();
    for (index, item) in items.iter().enumerate() {
        match read_block(item, index) {
            Ok(input) => inputs.push(input),
            Err(found) => faults.extend(found),
        }
    }
    if faults.is_empty() {
        Ok(inputs)
    } else {
        Err(faults)
    }
}

fn read_block(item: &Value, index: usize) -> Result<Input, Vec<Fault>> {
    let place = format!("#{}", index + 1);
    let Value::Object(map) = item else {
        return Err(vec![Fault {
            block: place,
            field: String::new(),
            message: "a block is a mapping".to_owned(),
        }]);
    };
    let named = map.get("id").and_then(Value::as_str).map(str::to_owned);
    let mut fields = Fields::new(map, named.unwrap_or(place));
    let id = fields.handle("id");
    if id.is_none() && !map.contains_key("id") {
        fields.fault("id", "is required");
    }
    let kind = fields.word("kind", Kind::parse, &Kind::words());
    if kind.is_none() && !map.contains_key("kind") {
        fields.fault("kind", "is required");
    }
    let after = fields.handle("after");
    let stage = match kind {
        Some(Kind::Stage) | None => None,
        Some(_) => {
            let stage = fields.handle("stage");
            if stage.is_none() && !map.contains_key("stage") {
                fields.fault(
                    "stage",
                    "is required: every block but a stage names its stage",
                );
            }
            stage
        }
    };
    let request = kind.map(|kind| request(kind, &mut fields));
    // A block of no known kind has no fields to check the others against.
    let faults = if kind.is_some() {
        fields.finish()
    } else {
        fields.faults
    };
    match (id, request) {
        (Some(id), Some(request)) if faults.is_empty() => Ok(Input {
            id,
            stage,
            after,
            request,
        }),
        _ => Err(faults),
    }
}

fn request(kind: Kind, fields: &mut Fields<'_>) -> Request {
    match kind {
        Kind::Stage => Request::Stage(Stage {
            title: fields.text("title"),
            goal: fields.optional_text("goal"),
            status: fields
                .word("status", Status::parse, &Status::words())
                .unwrap_or(Status::Planned),
        }),
        Kind::Note => Request::Note(Note {
            title: fields.optional_text("title"),
            markdown: fields.plain_markdown("markdown").unwrap_or_else(|| {
                fields.fault("markdown", "is required");
                String::new()
            }),
            tone: fields.word("tone", Tone::parse, &Tone::words()),
            folded: fields.flag("folded"),
        }),
        Kind::Question => Request::Question(question(fields)),
        Kind::Approval => Request::Approval(Approval {
            title: fields.text("title"),
            markdown: fields.markdown("markdown"),
        }),
        Kind::Decision => Request::Decision {
            title: fields.text("title"),
            consequence: fields.markdown("consequence"),
            question: fields.handle("question"),
        },
        Kind::Task => Request::Task(Task {
            title: fields.text("title"),
            detail: fields.optional_text("detail"),
            state: fields
                .word("state", TaskState::parse, &TaskState::words())
                .unwrap_or(TaskState::Open),
            kind: fields
                .word("type", TaskType::parse, &TaskType::words())
                .unwrap_or(TaskType::Regular),
        }),
        Kind::Plan => Request::Plan(Plan {
            title: fields.text("title"),
            markdown: fields.markdown("markdown"),
            path: fields.relative_path("path"),
            plan_mode: fields.flag("planMode"),
        }),
        Kind::Loft => Request::Loft(loft(fields)),
    }
}

fn question(fields: &mut Fields<'_>) -> Question {
    let prompt = fields.text("prompt");
    let header = fields.text("header");
    let context = fields.optional_text("context");
    let multiple = fields.flag("multiple");
    let own_words = fields.flag("ownWords");
    let options = fields
        .list("options")
        .map(|items| {
            items
                .iter()
                .filter_map(|item| choice(fields, item))
                .collect()
        })
        .unwrap_or_default();
    let options: Vec<Choice> = options;
    if options.len() < 2
        && !fields
            .faults
            .iter()
            .any(|fault| fault.field.starts_with("options"))
    {
        fields.fault("options", "a question holds two or more options");
    }
    let mut labels: Vec<&str> = options.iter().map(|o| o.label.as_str()).collect();
    labels.sort_unstable();
    if labels.windows(2).any(|pair| pair[0] == pair[1]) {
        fields.fault("options", "the options repeat a label");
    }
    if !multiple && options.iter().filter(|o| o.recommended).count() > 1 {
        fields.fault(
            "options",
            "a question that takes one option recommends at most one",
        );
    }
    Question {
        prompt,
        header,
        context,
        options,
        multiple,
        own_words,
    }
}

fn choice(fields: &mut Fields<'_>, item: &Value) -> Option<Choice> {
    let Value::Object(map) = item else {
        fields.fault("options", "an option is a mapping with a label");
        return None;
    };
    let mut option = Fields::new(map, fields.block.clone());
    let label = option.text("label");
    let detail = option.optional_text("detail");
    let recommended = option.flag("recommended");
    let faults = option.finish();
    let clean = faults.is_empty();
    fields.faults.extend(faults.into_iter().map(|fault| Fault {
        field: format!("options.{}", fault.field),
        ..fault
    }));
    clean.then_some(Choice {
        label,
        detail,
        recommended,
    })
}

fn loft(fields: &mut Fields<'_>) -> LoftRequest {
    let view = fields.word("view", View::parse, &View::words());
    if view.is_none() && !fields.faults.iter().any(|fault| fault.field == "view") {
        fields.fault("view", "is required");
    }
    let note = fields.plain_markdown("note");
    let base = fields.optional_text("base");
    let decision = fields.optional_text("decision");
    let reasons: Vec<Reason> = fields
        .list("reasons")
        .map(|items| {
            items
                .iter()
                .filter_map(|item| reason(fields, item))
                .collect()
        })
        .unwrap_or_default();
    let view = view.unwrap_or(View::SpecChange);
    if (view == View::Decision) != decision.is_some() {
        fields.fault(
            "decision",
            "a `decision` view names its decision, and no other view names one",
        );
    }
    if base.is_some() && !matches!(view, View::SpecChange | View::Trace | View::Findings) {
        fields.fault(
            "base",
            "only a `spec-change`, `trace` or `findings` view compares with a base revision",
        );
    }
    if !reasons.is_empty() && view != View::Findings {
        fields.fault("reasons", "only a `findings` view gives reasons");
    }
    LoftRequest {
        view,
        note,
        base,
        decision,
        reasons,
    }
}

fn reason(fields: &mut Fields<'_>, item: &Value) -> Option<Reason> {
    let Value::Object(map) = item else {
        fields.fault("reasons", "a reason is a mapping of rule, key and reason");
        return None;
    };
    let mut entry = Fields::new(map, fields.block.clone());
    let rule = entry.text("rule");
    let key = if let Some(value) = entry.raw("key") {
        entry.key("key", value)
    } else {
        entry.fault("key", "is required");
        None
    };
    let text = entry.text("reason");
    let faults = entry.finish();
    let clean = faults.is_empty();
    fields.faults.extend(faults.into_iter().map(|fault| Fault {
        field: format!("reasons.{}", fault.field),
        ..fault
    }));
    key.filter(|_| clean).map(|key| Reason {
        rule,
        key,
        reason: text,
    })
}
