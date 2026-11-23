//! The errors of a sign-in, in words that say what mends them for the remote a command reached:
//! the document's own, mended by `loft remote` and the manifest's `remote`, or another
//! document's, mended by `loft interface login` and the handle's entry of `documents`. Both
//! wordings lie here, side by side, so that neither drifts from the other.

use super::{ENDPOINT_ENV, RemoteError, TOKEN_ENV};
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
                format!(
                    "{said}, or {TOKEN_ENV} names a token for one run, sent to the address {ENDPOINT_ENV} names"
                )
            } else {
                said
            }
        };
        Some(match self {
            Self::NoSignIn { address } => {
                let signs_in =
                    login.map_or_else(String::new, |login| format!(": {login} signs in"));
                token(format!("no sign-in to {address} {kept}{signs_in}"))
            }
            Self::SignInElsewhere { address, resource } => {
                let origin = network::origin(resource)
                    .map_or_else(|| resource.clone(), |origin| origin.to_string());
                let readdress = match remedy {
                    Remedy::Own => format!(
                        "`loft remote set <address>` with an address on {origin} names the \
                         remote by it and signs in to it"
                    ),
                    Remedy::Other { handle, .. } => format!(
                        "an address on {origin} in the entry of `documents` for {handle} names \
                         the remote by it"
                    ),
                };
                let again = login.map_or_else(String::new, |login| {
                    format!(
                        ", {login} signs in again once the remote serves its protected resource \
                         on the address's origin"
                    )
                });
                token(format!(
                    "the sign-in kept under {address} {kept} is for a protected resource on \
                     another origin than the address, {resource}: {readdress}{again}"
                ))
            }
            Self::Lapsed { address, error } => {
                let again =
                    login.map_or_else(String::new, |login| format!(": {login} signs in again"));
                token(format!(
                    "the sign-in to {address} has lapsed ({error}){again}"
                ))
            }
            Self::Refused { address, refusal } => {
                format!("{address}: {}", refusal.said(login.as_deref()))
            }
            Self::Unsigned {
                key,
                address,
                refusal,
                missing,
            } => format!(
                "{address}: {} to a request for the document {key} sent without a token: {}",
                refusal.answered(),
                missing.said(remedy)?
            ),
            _ => return None,
        })
    }
}
