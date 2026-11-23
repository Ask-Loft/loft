//! To whom a remote serves a document, as the answer to `GET <address>/head` names it, and the
//! line that says so before the document's first publish there.

use serde_json::Value;

use crate::out::errln;

/// To whom a remote serves a document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visibility {
    /// Anyone: its published states are read without signing in.
    Public,
    /// Those the remote admits alone.
    Private,
}

impl Visibility {
    /// The visibility the head answer `answer` names; none where it names none, or one Loft does
    /// not know.
    #[must_use]
    pub fn of(answer: &Value) -> Option<Self> {
        match answer["visibility"].as_str()? {
            "public" => Some(Self::Public),
            "private" => Some(Self::Private),
            _ => None,
        }
    }

    /// Says on standard error to whom the remote serves the document, where `visibility` names it.
    pub fn say(visibility: Option<Self>) {
        if let Some(visibility) = visibility {
            errln!("{}", visibility.said());
        }
    }

    /// The line that says to whom the remote serves the document.
    #[must_use]
    pub const fn said(self) -> &'static str {
        match self {
            Self::Public => {
                "the remote serves this document to anyone: anyone reads its published states \
                 without signing in"
            }
            Self::Private => {
                "the remote serves this document privately: only those it admits read it"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::Visibility;

    #[test]
    fn a_head_answer_names_a_visibility_loft_knows_or_none() {
        for (named, read) in [
            (json!("public"), Some(Visibility::Public)),
            (json!("private"), Some(Visibility::Private)),
            (json!("internal"), None),
            (json!(null), None),
            (json!(1), None),
        ] {
            let answer = json!({ "documentKey": "k", "visibility": named, "head": null });
            assert_eq!(Visibility::of(&answer), read, "{named}");
        }
        assert_eq!(Visibility::of(&json!({ "head": null })), None);
    }
}
