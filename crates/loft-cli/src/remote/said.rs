//! The errors of a sign-in, in words that say what mends them for the remote a command reached:
//! the document's own, mended by `loft remote` and the manifest's `remote`, or another
//! document's, mended by `loft interface login` and the handle's entry of `documents`. Both
//! wordings lie here, side by side, so that neither drifts from the other.

use super::{RemoteError, TOKEN_ENV};
use crate::escaped::Escaped;
use crate::network;

/// Whose remote an error came from, for the words of what mends it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Remedy<'a> {
    /// The document's own remote.
    Own,
    /// Another document's remote, under `handle`; `listed` where `documents` lists it, so that
    /// `loft interface login` can sign in to it.
    Other { handle: &'a str, listed: bool },
}

impl Remedy<'_> {
    /// The command that signs the person in to the remote, where one can.
    fn login(self) -> Option<String> {
        match self {
            Self::Own => Some("`loft remote login`".to_owned()),
            Self::Other {
                handle,
                listed: true,
            } => Some(format!("`loft interface login {handle}`")),
            Self::Other { listed: false, .. } => None,
        }
    }

    /// Where the sign-in is kept, said after the address it is kept under.
    fn kept(self) -> String {
        match self {
            Self::Own => "in loft/remotes.json".to_owned(),
            Self::Other { handle, .. } => format!("for {handle} in loft/remotes.json"),
        }
    }
}

impl RemoteError {
    /// The error in words for the remote `remedy` names, where it is an error of a sign-in or a
    /// refusal; none for any other error, which reads the same whoever's remote it came from.
    #[must_use]
    pub fn said(&self, remedy: Remedy<'_>) -> Option<String> {
        let login = remedy.login();
        let kept = remedy.kept();
        let own = remedy == Remedy::Own;
        let token = |said: String| {
            if own {
                format!("{said}, or {TOKEN_ENV} names a token for one run")
            } else {
                said
            }
        };
        // Every text the error holds is written escaped, as `Display` writes it.
        Some(match self {
            Self::NoSignIn { address } => {
                let address = Escaped(address);
                let signs_in =
                    login.map_or_else(String::new, |login| format!(": {login} signs in"));
                token(format!("no sign-in to {address} {kept}{signs_in}"))
            }
            Self::SignInElsewhere { address, resource } => {
                token(elsewhere(address, resource, remedy))
            }
            Self::Lapsed { address, error } => {
                let (address, error) = (Escaped(address), Escaped(error));
                let again =
                    login.map_or_else(String::new, |login| format!(": {login} signs in again"));
                token(format!(
                    "the sign-in to {address} has lapsed ({error}){again}"
                ))
            }
            Self::FormerResource { address, resource } => {
                let (address, resource) = (Escaped(address), Escaped(resource));
                let again =
                    login.map_or_else(String::new, |login| format!(": {login} signs in again"));
                token(format!(
                    "the sign-in to {address} was made for a former resource of the remote, \
                     {resource}{again}"
                ))
            }
            Self::NotCovered { address, resource } => {
                let (address, resource) = (Escaped(address), Escaped(resource));
                let again = login.map_or_else(String::new, |login| {
                    format!(": {login} signs in again to reach it")
                });
                token(format!(
                    "the sign-in to {address} does not cover {resource}{again}"
                ))
            }
            Self::Refused { address, refusal } => {
                let address = Escaped(address);
                format!("{address}: {}", refusal.said(login.as_deref()))
            }
            Self::Unsigned {
                key,
                address,
                refusal,
                missing,
            } => format!(
                "{}: {} to a request for the document {} sent without a token: {}",
                Escaped(address),
                refusal.answered(),
                Escaped(key),
                missing.said(remedy)?
            ),
            _ => return None,
        })
    }
}

/// The error for the sign-in kept under `address`, which is for `resource`, a protected resource
/// on another origin, in words for the remote `remedy` names, before the token that mends it for
/// one run. Every text it holds is written escaped.
fn elsewhere(address: &str, resource: &str, remedy: Remedy<'_>) -> String {
    let origin =
        network::origin(resource).map_or_else(|| resource.to_owned(), |origin| origin.to_string());
    let (address, origin) = (Escaped(address), Escaped(&origin));
    let resource = Escaped(resource);
    let readdress = match remedy {
        Remedy::Own => format!(
            "`loft remote set <address>` with an address on {origin} names the remote by it and \
             signs in to it"
        ),
        Remedy::Other { handle, .. } => format!(
            "an address on {origin} in the entry of `documents` for {handle} names the remote by \
             it"
        ),
    };
    let again = remedy.login().map_or_else(String::new, |login| {
        format!(
            ", {login} signs in again once the remote serves its protected resource on the \
             address's origin"
        )
    });
    format!(
        "the sign-in kept under {address} {} is for a protected resource on another origin than \
         the address, {resource}: {readdress}{again}",
        remedy.kept()
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Remedy, RemoteError};
    use crate::escaped::{ESCAPED, HOSTILE, shown};
    use crate::remote::{Refusal, TIMEOUT_ENV};

    #[test]
    fn a_resource_not_covered_is_said_with_the_login_of_whose_remote_it_is() {
        let (address, resource) = ("https://h.example/documents/one", "https://h.example/notes");
        let error = RemoteError::NotCovered {
            address: address.to_owned(),
            resource: resource.to_owned(),
        };
        let own = error.said(Remedy::Own).unwrap();
        let other = error
            .said(Remedy::Other {
                handle: "shelves",
                listed: true,
            })
            .unwrap();
        for said in [&own, &other] {
            for named in [address, resource, "does not cover"] {
                assert!(said.contains(named), "{named}: {said}");
            }
        }
        assert!(
            own.contains("`loft remote login` signs in again to reach it"),
            "{own}"
        );
        let login = "`loft interface login shelves` signs in again to reach it";
        assert!(other.contains(login), "{other}");
        assert!(!other.contains("loft remote"), "{other}");
    }

    /// Each error, with [`HOSTILE`] in every text it holds.
    fn errors() -> Vec<RemoteError> {
        let text = || HOSTILE.to_owned();
        let refusal = Refusal {
            status: 418,
            problem: json!({ "code": HOSTILE, "detail": HOSTILE }),
        };
        vec![
            RemoteError::NoAddress { key: text() },
            RemoteError::NoSignIn { address: text() },
            RemoteError::SignInElsewhere {
                address: text(),
                resource: text(),
            },
            RemoteError::SignInElsewhere {
                address: "https://remote.example/documents/one".to_owned(),
                resource: format!("https://elsewhere.example/documents#{HOSTILE}"),
            },
            RemoteError::Address(text()),
            RemoteError::NotHeld { key: text() },
            RemoteError::StateNotHeld {
                sequence: 1,
                state_root: text(),
            },
            RemoteError::Variable {
                name: TIMEOUT_ENV,
                value: text(),
            },
            RemoteError::Unreachable {
                address: text(),
                reason: text(),
            },
            RemoteError::Unreadable {
                address: text(),
                what: "head",
            },
            RemoteError::Refused {
                address: text(),
                refusal: refusal.clone(),
            },
            RemoteError::Unsigned {
                key: text(),
                address: text(),
                refusal,
                missing: Box::new(RemoteError::NoSignIn { address: text() }),
            },
            RemoteError::Lapsed {
                address: text(),
                error: text(),
            },
            RemoteError::FormerResource {
                address: text(),
                resource: text(),
            },
            RemoteError::NotCovered {
                address: text(),
                resource: text(),
            },
            RemoteError::SignIn {
                address: text(),
                reason: text(),
            },
        ]
    }

    #[test]
    fn every_text_a_remote_error_names_is_written_with_its_control_characters_escaped() {
        let other = Remedy::Other {
            handle: "shelves",
            listed: true,
        };
        for error in errors() {
            let mut said = vec![error.to_string()];
            said.extend(error.said(other));
            for said in said {
                assert!(shown(&said), "{error:?}: {said:?}");
                assert!(said.contains(ESCAPED), "{error:?}: {said}");
            }
        }
    }
}
