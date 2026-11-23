//! Which addresses a remote may have, and how an error names one: `https`, or `http` on the
//! loopback interface, an origin and a path alone, and never with the user information it was
//! given.

use super::RemoteError;
use crate::network;

/// `address` as a remote's address, if it may be contacted and is an origin and a path alone:
/// each request goes to the address followed by its endpoint's path, which a query or a
/// fragment would swallow, and user information would travel with every request and be printed
/// with every error.
///
/// # Errors
///
/// The address is neither `https` nor `http` on the loopback interface, or holds a query, a
/// fragment or user information.
pub fn checked(address: &str) -> Result<&str, RemoteError> {
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
/// The address is neither `https` nor `http` on the loopback interface.
pub fn contactable(address: &str) -> Result<&str, RemoteError> {
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
    use super::{RemoteError, checked, without_user};

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
