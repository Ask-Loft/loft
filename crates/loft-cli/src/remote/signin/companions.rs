//! The companion resources of a remote: the protected resources beside the remote's own that its
//! protected resource metadata lists in `companion_resources`, each by its identifier and its
//! kind, and that one sign-in covers (the accepted decision "A remote lists the companion
//! resources that one sign-in covers"). A sign-in keeps a listed resource only where it lies on
//! the origin of the remote's address, asking nothing of a listed resource on another origin, and
//! where its own protected resource metadata vouches for it: names it, names the remote's issuer
//! and lists its scopes, each a scope token of RFC 6749. It leaves out every other entry, says so
//! on standard error, and goes on. Each warning writes what the remote gave [`Escaped`].

use std::collections::HashSet;
use std::time::Duration;

use serde_json::Value;

use super::discovery::{RESOURCE_METADATA, document, get};
use crate::escaped::{self, Escaped};
use crate::network;
use crate::out::errln;
use crate::remote;

/// The member of the remote's protected resource metadata that lists its companion resources.
const COMPANION_RESOURCES: &str = "companion_resources";

/// The most entries of that member a sign-in reads.
const MOST: usize = 16;

/// Why a listed resource on another origin than the remote's address is left out.
const ELSEWHERE: &str = "it does not lie on the origin of the address";

/// A companion resource that a sign-in keeps: its identifier, its kind, and the scopes its own
/// metadata lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Companion {
    pub resource: String,
    pub kind: String,
    pub scopes: Vec<String>,
}

/// Each companion resource that `metadata`, the protected resource metadata of `own` for the
/// remote at `address`, lists and that a sign-in under `issuer` keeps, in the order listed. Each
/// entry left out is named on standard error, with why. Where the metadata of one cannot be
/// reached, it and each entry after it are left out, with one warning, and no metadata of theirs
/// is asked for, so that a stalled origin holds the sign-in back for one time limit at most.
pub fn kept(
    address: &str,
    own: &str,
    issuer: &str,
    metadata: &Value,
    timeout: Duration,
) -> Vec<Companion> {
    let (listed, warnings) = listed(address, own, metadata);
    let address = Escaped(address);
    for warning in warnings {
        errln!("warning: the sign-in to {address} {warning}");
    }
    let mut held = Vec::new();
    for (resource, kind) in listed {
        let named = named(&resource, &kind);
        match learned(&resource, issuer, timeout) {
            Learned::Vouched(scopes) => held.push(Companion {
                resource,
                kind,
                scopes,
            }),
            Learned::Unvouched(why) => {
                errln!("warning: the sign-in to {address} leaves out {named}: {why}");
            }
            Learned::Unreached(why) => {
                errln!(
                    "warning: the sign-in to {address} leaves out {named} and each companion \
                     resource listed after it: {why}"
                );
                break;
            }
        }
    }
    held
}

/// The entry of `resource` and `kind` as a warning names it, each written [`Escaped`].
fn named(resource: &str, kind: &str) -> String {
    format!("{} ({})", Escaped(resource), Escaped(kind))
}

/// What a sign-in learns of a listed companion resource from its own protected resource
/// metadata.
enum Learned {
    /// The metadata vouches for it, with these scopes.
    Vouched(Vec<String>),
    /// The metadata answered, or lies where it may not be asked for, and does not vouch for it:
    /// why, with what the remote gave written escaped.
    Unvouched(String),
    /// The metadata cannot be reached: why, written escaped.
    Unreached(String),
}

/// What the protected resource metadata of the companion resource `resource` says of it under
/// the authorization server `issuer`, asked for within `timeout`.
fn learned(resource: &str, issuer: &str, timeout: Duration) -> Learned {
    let Some(at) = metadata_address(resource) else {
        return Learned::Unvouched(ELSEWHERE.to_owned());
    };
    match get(&at, timeout) {
        Err(error) => Learned::Unreached(format!(
            "its metadata cannot be reached: {}: {}",
            Escaped(&at),
            Escaped(&error)
        )),
        Ok(reply) => {
            // `document` names the address as it is; `vouched` writes what it names escaped.
            let found = document(&at, &reply).map_err(|why| Escaped(&why).to_string());
            match found.and_then(|found| vouched(resource, issuer, &found)) {
                Ok(scopes) => Learned::Vouched(scopes),
                Err(why) => Learned::Unvouched(why),
            }
        }
    }
}

/// The identifier and the kind of each entry of the `companion_resources` that `metadata`, the
/// protected resource metadata of `own` for the remote at `address`, lists and that a sign-in
/// asks about, in order; and, for each warning, what the sign-in leaves out and why, each entry
/// [`named`]. No entry of a member that is not a list is read, nor any entry after the first
/// [`MOST`].
fn listed(address: &str, own: &str, metadata: &Value) -> (Vec<(String, String)>, Vec<String>) {
    let (mut read, mut warnings) = (Vec::new(), Vec::new());
    let Some(member) = metadata.get(COMPANION_RESOURCES) else {
        return (read, warnings);
    };
    let Some(entries) = member.as_array() else {
        let why = format!("reads no companion resource: `{COMPANION_RESOURCES}` is not a list");
        return (read, vec![why]);
    };
    let mut seen: Vec<(String, String)> = Vec::new();
    for entry in entries.iter().take(MOST) {
        let Some((resource, kind)) = shape(entry) else {
            warnings.push(format!(
                "leaves out an entry of `{COMPANION_RESOURCES}` that is not an object of a string \
                 `resource` and a non-empty string `kind`"
            ));
            continue;
        };
        let why = left_out(address, own, &seen, &resource, &kind);
        seen.push((resource.clone(), kind.clone()));
        match why {
            Some(why) => warnings.push(format!("leaves out {}: {why}", named(&resource, &kind))),
            None => read.push((resource, kind)),
        }
    }
    if entries.len() > MOST {
        warnings.push(format!(
            "leaves out every entry of `{COMPANION_RESOURCES}` after the first {MOST}"
        ));
    }
    (read, warnings)
}

/// The `resource` and the `kind` of `entry`, where it is an object of a string `resource` and a
/// non-empty string `kind`.
fn shape(entry: &Value) -> Option<(String, String)> {
    let resource = entry.get("resource")?.as_str()?;
    let kind = entry
        .get("kind")?
        .as_str()
        .filter(|kind| !kind.is_empty())?;
    Some((resource.to_owned(), kind.to_owned()))
}

/// Why the entry of `resource` and `kind` that the protected resource metadata of `own` lists for
/// the remote at `address`, after the entries `seen`, is left out before anything is asked of it:
/// its identifier is malformed or holds a character other than visible ASCII, which an address
/// that a remote's metadata names may not hold, it is the remote's own, an entry before it names
/// the same resource or kind, or it lies on another origin. Nothing for an entry that a sign-in
/// asks about.
fn left_out(
    address: &str,
    own: &str,
    seen: &[(String, String)],
    resource: &str,
    kind: &str,
) -> Option<&'static str> {
    if malformed(resource) {
        Some("its identifier holds user information or a fragment")
    } else if !remote::visible(resource) {
        Some("its identifier holds a character other than visible ASCII")
    } else if resource == own {
        Some("it is the remote's own protected resource")
    } else if seen
        .iter()
        .any(|(before, kind_before)| before == resource || kind_before == kind)
    {
        Some("an entry before it names the same resource or kind")
    } else if !network::same_origin(resource, address) {
        Some(ELSEWHERE)
    } else {
        None
    }
}

/// Whether `resource` holds a fragment, which the identifier of a protected resource never holds
/// (RFC 8707, section 2), or user information before its host.
fn malformed(resource: &str) -> bool {
    let authority = resource.split_once("://").map_or("", |(_, rest)| {
        rest.split(['/', '?', '#']).next().unwrap_or_default()
    });
    resource.contains('#') || authority.contains('@')
}

/// The scopes that `metadata`, the protected resource metadata read for the companion resource
/// `resource`, lists, where it vouches for it under the authorization server `issuer`: it names
/// the resource and the issuer and lists one scope or more, each a scope token. Otherwise why it
/// does not, with what the metadata names written [`Escaped`], and the first value that is not a
/// scope token as [`escaped::value`] writes it.
fn vouched(resource: &str, issuer: &str, metadata: &Value) -> Result<Vec<String>, String> {
    match metadata["resource"].as_str() {
        None => return Err("its metadata names no resource".to_owned()),
        Some(named) if named != resource => {
            let named = Escaped(named);
            return Err(format!("its metadata names another resource, {named}"));
        }
        Some(_) => {}
    }
    let servers = metadata["authorization_servers"].as_array();
    if !servers.is_some_and(|servers| servers.iter().any(|server| server == issuer)) {
        let issuer = Escaped(issuer);
        return Err(format!(
            "its metadata does not name the authorization server {issuer}"
        ));
    }
    let scopes = metadata["scopes_supported"]
        .as_array()
        .filter(|scopes| !scopes.is_empty())
        .ok_or_else(|| "its metadata lists no scope".to_owned())?;
    if let Some(value) = scopes.iter().find(|value| !scope_token(value)) {
        let value = escaped::value(value);
        return Err(format!("its metadata lists {value}, which is not a scope"));
    }
    let scopes = scopes.iter().filter_map(Value::as_str);
    Ok(scopes.map(str::to_owned).collect())
}

/// Whether `value` is a scope token of RFC 6749, section 3.3: a string of one character or more,
/// each `%x21 / %x23-5B / %x5D-7E`, so neither a space, a `"`, a `\`, a control character nor a
/// character outside ASCII.
fn scope_token(value: &Value) -> bool {
    value.as_str().is_some_and(|scope| {
        !scope.is_empty()
            && scope
                .bytes()
                .all(|byte| matches!(byte, 0x21 | 0x23..=0x5B | 0x5D..=0x7E))
    })
}

/// Where RFC 9728, section 3.1, places the protected resource metadata of the resource
/// `resource`: its origin, then `/.well-known/oauth-protected-resource`, then its path without a
/// terminating `/`, then its query. Nothing for an identifier that is not an absolute address.
fn metadata_address(resource: &str) -> Option<String> {
    let (origin, path, query) = network::origin_path_and_query(resource)?;
    let path = path.trim_end_matches('/');
    Some(format!("{origin}{RESOURCE_METADATA}{path}{query}"))
}

/// The scope a sign-in asks for: `own`, the remote's scope, as it is, where no companion resource
/// is kept; otherwise each scope of `own`, then each scope of each companion resource kept, in
/// order, each once, separated by single spaces.
#[must_use]
pub fn asked(own: &str, companions: &[Companion]) -> String {
    if companions.is_empty() {
        return own.to_owned();
    }
    let listed = companions.iter().flat_map(|companion| &companion.scopes);
    let (mut seen, mut scopes) = (HashSet::new(), Vec::new());
    for scope in own.split(' ').chain(listed.map(String::as_str)) {
        if !scope.is_empty() && seen.insert(scope) {
            scopes.push(scope);
        }
    }
    scopes.join(" ")
}

#[cfg(test)]
mod escapes;
#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{Companion, asked, listed, metadata_address, vouched};

    /// The address of a remote's document, and the remote's own protected resource.
    pub(super) const ADDRESS: &str = "https://h.example/documents/one";
    pub(super) const OWN: &str = "https://h.example/documents";

    /// The identifier and the kind of each entry that a listing read.
    pub(super) fn read_as_text(read: &[(String, String)]) -> Vec<(&str, &str)> {
        read.iter()
            .map(|(resource, kind)| (resource.as_str(), kind.as_str()))
            .collect()
    }

    #[test]
    fn a_listing_keeps_well_formed_distinct_entries_of_other_resources_on_the_address_s_origin_in_order()
     {
        let metadata = json!({ "companion_resources": [
            { "resource": "https://h.example/notes", "kind": "notes" },
            { "resource": "https://h.example/notes", "kind": "again" },
            { "resource": "https://h.example/other", "kind": "notes" },
            { "resource": OWN, "kind": "own" },
            { "resource": "https://h.example/nameless" },
            { "resource": "https://h.example/empty", "kind": "" },
            "https://h.example/text",
            { "resource": "https://h.example/part#one", "kind": "fragment" },
            { "resource": "https://someone@h.example/x", "kind": "user" },
            { "resource": "https://g.example/x", "kind": "elsewhere" },
            { "resource": "/relative", "kind": "relative" },
            { "resource": "https://h.example/mail/inbox?view=all", "kind": "inbox" },
        ]});
        let (read, warnings) = listed(ADDRESS, OWN, &metadata);
        assert_eq!(
            read_as_text(&read),
            [
                ("https://h.example/notes", "notes"),
                ("https://h.example/mail/inbox?view=all", "inbox"),
            ]
        );
        let repeated = "an entry before it names the same resource or kind";
        let malformed = "its identifier holds user information or a fragment";
        let elsewhere = "it does not lie on the origin of the address";
        let unnamed = "leaves out an entry of `companion_resources` that is not an object of a \
                       string `resource` and a non-empty string `kind`";
        assert_eq!(
            warnings,
            [
                format!("leaves out https://h.example/notes (again): {repeated}"),
                format!("leaves out https://h.example/other (notes): {repeated}"),
                format!("leaves out {OWN} (own): it is the remote's own protected resource"),
                unnamed.to_owned(),
                unnamed.to_owned(),
                unnamed.to_owned(),
                format!("leaves out https://h.example/part#one (fragment): {malformed}"),
                format!("leaves out https://someone@h.example/x (user): {malformed}"),
                format!("leaves out https://g.example/x (elsewhere): {elsewhere}"),
                format!("leaves out /relative (relative): {elsewhere}"),
            ]
        );
    }

    #[test]
    fn a_listing_reads_at_most_sixteen_entries() {
        let entries: Vec<Value> = (1..=17)
            .map(|n| json!({ "resource": format!("https://h.example/r{n}"), "kind": format!("k{n}") }))
            .collect();
        let (read, warnings) = listed(ADDRESS, OWN, &json!({ "companion_resources": entries }));
        assert_eq!(read.len(), 16);
        let last = read.last().map(|(resource, _)| resource.as_str());
        assert_eq!(last, Some("https://h.example/r16"));
        assert_eq!(
            warnings,
            ["leaves out every entry of `companion_resources` after the first 16"]
        );
        let sixteen = json!({ "companion_resources": entries[..16] });
        let (read, warnings) = listed(ADDRESS, OWN, &sixteen);
        assert_eq!((read.len(), warnings.len()), (16, 0));
    }

    #[test]
    fn a_member_that_is_not_a_list_lists_nothing() {
        for member in [
            json!({ "resource": "https://h.example/notes", "kind": "notes" }),
            json!("https://h.example/notes"),
        ] {
            let (read, warnings) = listed(ADDRESS, OWN, &json!({ "companion_resources": member }));
            assert!(read.is_empty(), "{member}");
            assert_eq!(
                warnings,
                ["reads no companion resource: `companion_resources` is not a list"],
                "{member}"
            );
        }
        for metadata in [json!({}), json!({ "companion_resources": [] })] {
            let none = (Vec::new(), Vec::new());
            assert_eq!(listed(ADDRESS, OWN, &metadata), none, "{metadata}");
        }
    }

    #[test]
    fn the_metadata_of_a_companion_resource_lies_where_rfc_9728_places_it() {
        let well_known = "https://h.example/.well-known/oauth-protected-resource";
        for (resource, rest) in [
            ("https://h.example/notes", "/notes"),
            (
                "https://h.example/mail/inbox?view=all",
                "/mail/inbox?view=all",
            ),
            ("https://h.example/notes/", "/notes"),
            ("https://H.Example:443/notes", "/notes"),
            ("https://h.example", ""),
            ("https://h.example/", ""),
        ] {
            let expected = format!("{well_known}{rest}");
            assert_eq!(metadata_address(resource), Some(expected), "{resource}");
        }
        assert_eq!(metadata_address("/notes"), None);
    }

    #[test]
    fn a_companion_resource_is_kept_only_where_its_metadata_vouches_for_it() {
        let (resource, issuer) = ("https://h.example/notes", "https://h.example/auth");
        let faithful = json!({
            "resource": resource,
            "authorization_servers": ["https://other.example/auth", issuer],
            "scopes_supported": ["notes", "notes.archive"],
        });
        let scopes = |listed: &[&str]| -> Result<Vec<String>, String> {
            Ok(listed.iter().map(|scope| (*scope).to_owned()).collect())
        };
        assert_eq!(
            vouched(resource, issuer, &faithful),
            scopes(&["notes", "notes.archive"])
        );
        let with = |name: &str, value: Value| {
            let mut metadata = faithful.clone();
            metadata[name] = value;
            metadata
        };
        let without = |name: &str| {
            let mut metadata = faithful.clone();
            metadata.as_object_mut().unwrap().remove(name);
            metadata
        };
        let unnamed_issuer =
            format!("its metadata does not name the authorization server {issuer}");
        for (metadata, said) in [
            (
                with("resource", json!("https://h.example/other")),
                "its metadata names another resource, https://h.example/other",
            ),
            (without("resource"), "its metadata names no resource"),
            (
                with(
                    "authorization_servers",
                    json!([format!("{issuer}/elsewhere")]),
                ),
                &unnamed_issuer,
            ),
            (without("authorization_servers"), &unnamed_issuer),
            (without("scopes_supported"), "its metadata lists no scope"),
            (
                with("scopes_supported", json!([])),
                "its metadata lists no scope",
            ),
        ] {
            let refused = Err(said.to_owned());
            assert_eq!(vouched(resource, issuer, &metadata), refused, "{metadata}");
        }
        // A value that is not a scope token is shown escaped: a string between double quotes, any
        // other value as JSON.
        for (value, shown) in [
            (json!(""), r#""""#),
            (json!("a b"), r#""a b""#),
            (json!("a\tb"), r#""a\u{9}b""#),
            (json!("a\"b"), r#""a"b""#),
            (json!("a\\b"), r#""a\\b""#),
            (json!("é"), r#""é""#),
            (json!(3), "3"),
            (json!(["\u{9b}"]), r#"["\u{9b}"]"#),
        ] {
            let metadata = with("scopes_supported", json!(["notes", value]));
            let refused = Err(format!("its metadata lists {shown}, which is not a scope"));
            assert_eq!(vouched(resource, issuer, &metadata), refused, "{value}");
        }
        let marks = with("scopes_supported", json!(["!", "#", "[", "]", "~"]));
        assert_eq!(
            vouched(resource, issuer, &marks),
            scopes(&["!", "#", "[", "]", "~"])
        );
    }

    #[test]
    fn the_scope_asked_for_is_the_remote_s_then_each_new_scope_of_a_kept_companion_resource() {
        for own in ["documents", "documents  profile", ""] {
            assert_eq!(asked(own, &[]), own, "{own:?}");
        }
        let companion = |kind: &str, scopes: &[&str]| Companion {
            resource: format!("https://h.example/{kind}"),
            kind: kind.to_owned(),
            scopes: scopes.iter().map(|scope| (*scope).to_owned()).collect(),
        };
        let companions = [
            companion("notes", &["notes", "documents", "notes.archive"]),
            companion("inbox", &["inbox", "notes"]),
        ];
        assert_eq!(
            asked("documents profile", &companions),
            "documents profile notes notes.archive inbox"
        );
    }
}
