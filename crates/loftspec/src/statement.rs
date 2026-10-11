//! A Requirement's sentence (D-11 F-10) and the fixed templates that render it (D-11 F-11).

use crate::ears::{self, Form};
use crate::text::token_ranges;

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

impl EarsForm {
    /// The form's name as the text form writes it.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Ubiquitous { .. } => "ubiquitous",
            Self::EventDriven { .. } => "event_driven",
            Self::StateDriven { .. } => "state_driven",
            Self::Unwanted { .. } => "unwanted",
            Self::Optional { .. } => "optional",
        }
    }

    /// The sentence the form's fixed template yields (RQ-40).
    #[must_use]
    pub fn render(&self) -> String {
        match self {
            Self::Ubiquitous { system, shall } => format!("{system} shall {shall}"),
            Self::EventDriven {
                when,
                system,
                shall,
            } => {
                format!("When {when}, {system} shall {shall}")
            }
            Self::StateDriven {
                r#while,
                system,
                shall,
            } => {
                format!("While {while}, {system} shall {shall}")
            }
            Self::Unwanted {
                r#if,
                system,
                shall,
            } => {
                format!("If {if}, then {system} shall {shall}")
            }
            Self::Optional {
                r#where,
                system,
                shall,
            } => {
                format!("Where {where}, {system} shall {shall}")
            }
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
        let condition = |word: &str| lead.strip_prefix(word).map(str::to_owned);
        Some(match parse.form {
            Form::Ubiquitous => return None,
            Form::EventDriven => Self::EventDriven {
                when: condition("When ")?,
                system,
                shall,
            },
            Form::StateDriven => Self::StateDriven {
                r#while: condition("While ")?,
                system,
                shall,
            },
            Form::Unwanted => Self::Unwanted {
                r#if: condition("If ")?,
                system,
                shall,
            },
            Form::Optional => Self::Optional {
                r#where: condition("Where ")?,
                system,
                shall,
            },
        })
    }

    /// Every slot, in template order.
    fn slots(&self) -> Vec<&str> {
        match self {
            Self::Ubiquitous { system, shall } => vec![system, shall],
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
            } => vec![condition, system, shall],
        }
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
