//! Which addresses a remote may have, and how an error names one: visible ASCII alone, `https`,
//! or `http` on the loopback interface, an origin and a path alone, and never with the user
//! information it was given.

use super::RemoteError;
use crate::escaped::Escaped;
use crate::network;

impl RemoteError {
    /// The error in words where it refuses an address: the address, written [`Escaped`], and
    /// what a remote's address is or holds. Nothing for any other error.
    pub(super) fn refused_address(&self) -> Option<String> {
        let (address, rule) = match self {
            Self::Address(address) => (
                address,
                "is `https`, or `http` on the loopback interface".to_owned(),
            ),
            Self::Characters(address) => (
                address,
                "holds visible ASCII characters alone, with every other character \
                 percent-encoded"
                    .to_owned(),
            ),
            Self::AddressPart { address, part } => (
                address,
                format!(
                    "holds no {part}: each request goes to the address followed by its \
                     endpoint's path"
                ),
            ),
            _ => return None,
        };
        Some(format!("{}: a remote's address {rule}", Escaped(address)))
    }
}

/// Whether `address` holds the visible characters of ASCII alone, `!` to `~`, as the address of
/// a remote or of a protected resource beside it does, every other character percent-encoded.
#[must_use]
pub fn visible(address: &str) -> bool {
    address.bytes().all(|byte| byte.is_ascii_graphic())
}

/// Refuses `address` where it holds a character other than the visible characters of ASCII,
/// naming it without its user information. The characters are held first, before anything
/// reads the address, so that the address a person reads is the address contacted.
fn in_visible_ascii(address: &str) -> Result<(), RemoteError> {
    if visible(address) {
        Ok(())
    } else {
        Err(RemoteError::Characters(without_user(address)))
    }
}

/// `address` as a remote's address, if it may be contacted and is an origin and a path alone:
/// each request goes to the address followed by its endpoint's path, which a query or a
/// fragment would swallow, and user information would travel with every request and be printed
/// with every error.
///
/// # Errors
///
/// The address holds a character other than the visible characters of ASCII, is neither `https`
/// nor `http` on the loopback interface, or holds a query, a fragment or user information.
pub fn checked(address: &str) -> Result<&str, RemoteError> {
    in_visible_ascii(address)?;
    let uri =
        network::allowed(address).ok_or_else(|| RemoteError::Address(without_user(address)))?;
    let user = (uri.authority()).is_some_and(|authority| authority.as_str().contains('@'));
    let part = if address.contains('#') {
        "fragment"
    } else if address.contains('?') {
        "query"
    } else if user {
        "user information"
    } else {
        return Ok(address);
    };
    Err(RemoteError::AddressPart {
        address: without_user(address),
        part,
    })
}

/// `address` as the address of a request, if it may be contacted, whatever else it holds: an
/// authorization server's endpoint may hold a query.
///
/// # Errors
///
/// The address holds a character other than the visible characters of ASCII, or is neither
/// `https` nor `http` on the loopback interface.
pub fn contactable(address: &str) -> Result<&str, RemoteError> {
    in_visible_ascii(address)?;
    network::allowed(address)
        .map(|_| address)
        .ok_or_else(|| RemoteError::Address(without_user(address)))
}

/// `address` without the user information before its host, so that no error prints a password
/// an address was given. Where the last `@` does not lie where user information ends, in an
/// address that does not read as one, what stands before it may still be a password with a
/// `/`, a `?` or a `#` in it, or one given without a scheme: it is left out, and `…@` stands for
/// it.
fn without_user(address: &str) -> String {
    let Some(at) = address.rfind('@') else {
        return address.to_owned();
    };
    let (before, after) = (&address[..at], &address[at + 1..]);
    match before.split_once("://") {
        Some((scheme, user)) if !user.contains(['/', '?', '#']) => format!("{scheme}://{after}"),
        Some((scheme, _)) => format!("{scheme}://…@{after}"),
        None => format!("…@{after}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{RemoteError, checked, contactable, without_user};

    #[test]
    fn an_error_names_an_address_without_its_user_information() {
        for (address, shown) in [
            ("https://host.example/doc", "https://host.example/doc"),
            (
                "https://ci:secret@host.example/doc",
                "https://host.example/doc",
            ),
            ("https://host.example/users/@me", "https://…@me"),
            (
                "https://ci:se#cret@host.example/doc",
                "https://…@host.example/doc",
            ),
            (
                "https://ci:se/cret@host.example/doc",
                "https://…@host.example/doc",
            ),
            (
                "https://ci:se?cret@host.example/doc",
                "https://…@host.example/doc",
            ),
            ("ci:secret@host.example/doc", "…@host.example/doc"),
        ] {
            assert_eq!(without_user(address), shown, "{address}");
        }
    }

    #[test]
    fn an_address_outside_visible_ascii_is_refused_for_its_characters_and_named_without_its_user_information()
     {
        for (address, named) in [
            (
                "https://ci:secret@host.example/doc?x#\u{1b}[0m",
                "https://host.example/doc?x#\u{1b}[0m",
            ),
            (
                "https://ci:se\u{202e}cret@host.example/doc",
                "https://host.example/doc",
            ),
            (
                "https://host.example/doc?b\u{fc}cher",
                "https://host.example/doc?b\u{fc}cher",
            ),
            (
                "https://host.example/doc#top\u{9b}",
                "https://host.example/doc#top\u{9b}",
            ),
        ] {
            let refused = RemoteError::Characters(named.to_owned());
            assert_eq!(checked(address), Err(refused.clone()), "{address}");
            assert_eq!(contactable(address), Err(refused), "{address}");
        }
    }

    #[test]
    fn an_address_with_an_at_sign_in_its_path_is_a_remote_s_address() {
        let address = "https://host.example/users/@me/doc";
        assert_eq!(checked(address), Ok(address));
        let refused = checked("https://ci:secret@host.example/doc?x").unwrap_err();
        assert_eq!(
            refused,
            RemoteError::AddressPart {
                address: "https://host.example/doc?x".to_owned(),
                part: "query",
            }
        );
    }
}
