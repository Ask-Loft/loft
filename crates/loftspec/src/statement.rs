//! A Requirement's sentence (D-11 F-10) and the fixed templates that render it (D-11 F-11).

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
}

impl Statement {
    /// The sentence: the text itself, or the form's rendering.
    #[must_use]
    pub fn render(&self) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::Form(form) => form.render(),
        }
    }
}
