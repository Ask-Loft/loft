//! A Requirement's sentence (F-10) and the fixed templates that render it (F-11).

use crate::ears;
pub use crate::ears::Form;
use crate::text::token_ranges;
use crate::wire::wire_names;

/// A Requirement's sentence, stored as plain text or as one of the five basic EARS forms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    Text(String),
    Form(EarsForm),
}

/// The five basic EARS forms with their slots (RQ-38). The `shall` slot holds everything after
/// the modal verb, including the closing punctuation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EarsForm {
    Ubiquitous {
        system: String,
        shall: String,
    },
    EventDriven {
        when: String,
        system: String,
        shall: String,
    },
    StateDriven {
        r#while: String,
        system: String,
        shall: String,
    },
    Unwanted {
        r#if: String,
        system: String,
        shall: String,
    },
    Optional {
        r#where: String,
        system: String,
        shall: String,
    },
}

wire_names!(Form {
    Ubiquitous => "ubiquitous",
    EventDriven => "event_driven",
    StateDriven => "state_driven",
    Unwanted => "unwanted",
    Optional => "optional",
});

impl Form {
    /// The form's slot names in template order, as the text form writes them: the condition
    /// slot, named by the template's opening word in lowercase, then `system` and `shall`.
    #[must_use]
    pub const fn slots(self) -> &'static [&'static str] {
        match self {
            Self::Ubiquitous => &["system", "shall"],
            Self::EventDriven => &["when", "system", "shall"],
            Self::StateDriven => &["while", "system", "shall"],
            Self::Unwanted => &["if", "system", "shall"],
            Self::Optional => &["where", "system", "shall"],
        }
    }

    /// The name of the condition slot; none for the ubiquitous form.
    #[must_use]
    pub const fn condition(self) -> Option<&'static str> {
        match self.slots() {
            [condition, _, _] => Some(condition),
            _ => None,
        }
    }
}

impl EarsForm {
    /// The form, without its slots.
    #[must_use]
    pub const fn kind(&self) -> Form {
        match self {
            Self::Ubiquitous { .. } => Form::Ubiquitous,
            Self::EventDriven { .. } => Form::EventDriven,
            Self::StateDriven { .. } => Form::StateDriven,
            Self::Unwanted { .. } => Form::Unwanted,
            Self::Optional { .. } => Form::Optional,
        }
    }

    /// A form from its slots; the ubiquitous form takes no condition and ignores it.
    #[must_use]
    pub fn from_slots(form: Form, condition: String, system: String, shall: String) -> Self {
        match form {
            Form::Ubiquitous => Self::Ubiquitous { system, shall },
            Form::EventDriven => Self::EventDriven {
                when: condition,
                system,
                shall,
            },
            Form::StateDriven => Self::StateDriven {
                r#while: condition,
                system,
                shall,
            },
            Form::Unwanted => Self::Unwanted {
                r#if: condition,
                system,
                shall,
            },
            Form::Optional => Self::Optional {
                r#where: condition,
                system,
                shall,
            },
        }
    }

    /// The sentence the form's fixed template yields (RQ-40).
    #[must_use]
    pub fn render(&self) -> String {
        self.render_with_modal("shall")
    }

    /// The sentence the form's template yields with its modal word written as `modal`, such as
    /// `SHALL` for a projection whose readers look for the upper-case word.
    #[must_use]
    pub fn render_with_modal(&self, modal: &str) -> String {
        match self {
            Self::Ubiquitous { system, shall } => format!("{system} {modal} {shall}"),
            Self::EventDriven {
                when,
                system,
                shall,
            } => format!("When {when}, {system} {modal} {shall}"),
            Self::StateDriven {
                r#while,
                system,
                shall,
            } => format!("While {while}, {system} {modal} {shall}"),
            Self::Unwanted {
                r#if,
                system,
                shall,
            } => format!("If {if}, then {system} {modal} {shall}"),
            Self::Optional {
                r#where,
                system,
                shall,
            } => format!("Where {where}, {system} {modal} {shall}"),
        }
    }

    /// Cuts `sentence` into the slots of the EARS form it parses as, at the template's anchors:
    /// the prefix word, the comma before the system, `then`, and `shall`. The slots need not
    /// render back to the sentence; [`Statement::parse`] keeps them only when they do.
    fn cut(sentence: &str) -> Option<Self> {
        let ranges = token_ranges(sentence);
        let parse = ears::parse(sentence, &ranges)?;
        let shall_token = &ranges[parse.response.start - 1];
        let shall = sentence[shall_token.end..].strip_prefix(' ')?.to_owned();
        let head = sentence[..shall_token.start].strip_suffix(' ')?;
        let Some(precondition) = parse.precondition else {
            return Some(Self::Ubiquitous {
                system: head.to_owned(),
                shall,
            });
        };
        // The precondition ends at the `, ` between its last token and the next one, which is
        // the system's first token or `then`; punctuation such as a backtick may sit on either
        // side of the comma.
        let gap = ranges[precondition.end - 1].end..ranges[precondition.end].start;
        let comma = gap.start + sentence[gap].find(", ")?;
        let lead = &sentence[..comma];
        let rest = head.get(comma + 2..)?;
        let system = if parse.form == Form::Unwanted {
            rest.strip_prefix("then ")?
        } else {
            rest
        }
        .to_owned();
        let word = match parse.form {
            Form::Ubiquitous => return None,
            Form::EventDriven => "When ",
            Form::StateDriven => "While ",
            Form::Unwanted => "If ",
            Form::Optional => "Where ",
        };
        let condition = lead.strip_prefix(word)?.to_owned();
        Some(Self::from_slots(parse.form, condition, system, shall))
    }

    /// The condition slot, when the form has one, then `system` and `shall`.
    #[must_use]
    pub fn parts(&self) -> (Option<&str>, &str, &str) {
        match self {
            Self::Ubiquitous { system, shall } => (None, system, shall),
            Self::EventDriven {
                when: condition,
                system,
                shall,
            }
            | Self::StateDriven {
                r#while: condition,
                system,
                shall,
            }
            | Self::Unwanted {
                r#if: condition,
                system,
                shall,
            }
            | Self::Optional {
                r#where: condition,
                system,
                shall,
            } => (Some(condition), system, shall),
        }
    }

    /// Every slot, in template order.
    fn slots(&self) -> Vec<&str> {
        let (condition, system, shall) = self.parts();
        condition.into_iter().chain([system, shall]).collect()
    }
}

impl Statement {
    /// Reads a sentence as slots when it is one of the five basic EARS forms, each slot a
    /// non-empty phrase without surrounding white space, and the form's template renders the
    /// slots back to the sentence byte for byte (RQ-52); as text otherwise.
    #[must_use]
    pub fn parse(sentence: &str) -> Self {
        EarsForm::cut(sentence)
            .filter(|form| {
                form.slots()
                    .iter()
                    .all(|slot| !slot.is_empty() && slot.trim() == *slot)
            })
            .filter(|form| form.render() == sentence)
            .map_or_else(|| Self::Text(sentence.to_owned()), Self::Form)
    }

    /// The sentence: the text itself, or the form's rendering.
    #[must_use]
    pub fn render(&self) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::Form(form) => form.render(),
        }
    }
}
