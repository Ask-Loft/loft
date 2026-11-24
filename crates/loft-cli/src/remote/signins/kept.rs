//! A sign-in as `loft/remotes.json` keeps it: the six members every `loft` reads, for the
//! remote's own protected resource and the one refresh token; the issuer of the authorization
//! server and the companion resources, for a sign-in over them, read leniently; and the record of
//! a refresh whose answer has not been kept, read strictly. Only a sign-in for a protected
//! resource on the address's origin is one for that address, and a refresh changes only the
//! members it replaces.

use std::str::FromStr as _;

use loftspec::publish::PublishedAt as Time;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

use super::MARGIN;
use crate::network;
use crate::remote::RemoteError;
use crate::remote::signin::Issued;

/// The member that records the protected resource of a refresh whose answer has not been kept.
const REFRESHING: &str = "refreshing";

/// The member that lists the companion resources of a sign-in over them.
const RESOURCES: &str = "resources";

/// Why a sign-in whose record of a refresh does not read has lapsed.
const UNREADABLE_RECORD: &str = "a recorded refresh does not read";

/// A sign-in to one remote: the token endpoint and resource it refreshes with, the client it was
/// issued to, its tokens, and when its access token expires, in UTC; for a sign-in over companion
/// resources, also the issuer of the authorization server and each companion resource. A sign-in
/// over the remote's protected resource alone is kept with neither, as an earlier `loft` keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignIn {
    pub token_endpoint: String,
    pub resource: String,
    pub client_id: String,
    pub access_token: String,
    pub expires_at: String,
    pub refresh_token: String,
    #[serde(
        default,
        deserialize_with = "lenient",
        skip_serializing_if = "Option::is_none"
    )]
    pub issuer: Option<String>,
    #[serde(
        default,
        deserialize_with = "lenient",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub resources: Vec<Covered>,
}

/// A companion resource that a sign-in asked for: its identifier, its kind, and the access token
/// last issued for it, with when it expires, once a refresh has issued one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Covered {
    pub resource: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

/// A member that does not read, read as absent: what it would cover is then not covered.
fn lenient<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned + Default,
{
    Ok(serde_json::from_value(Value::deserialize(deserializer)?).unwrap_or_default())
}

/// Whether `expires_at` falls within [`MARGIN`] of `now`, or does not read.
fn expiring(expires_at: &str, now: u64) -> bool {
    Time::from_str(expires_at).map_or(true, |expires| expires.unix_seconds() <= now + MARGIN)
}

/// The sign-in that an entry of `loft/remotes.json` keeps for an address, the protected resource
/// of a refresh it records, and whether it records each refresh.
pub(super) struct Kept {
    pub(super) sign_in: SignIn,
    pub(super) refreshing: Option<String>,
    /// Whether the entry holds `resources`, whether it reads or not: a sign-in over companion
    /// resources, whose every refresh is recorded.
    pub(super) records: bool,
}

impl Kept {
    /// The sign-in that `entries`, the entries of `loft/remotes.json` by address, keep for
    /// `address`, where it reads as one and its resource lies on the address's origin.
    ///
    /// # Errors
    ///
    /// There is no entry, or it is not a JSON object, or it does not read as a sign-in, or its
    /// resource reads as no `https` or `http` address. Or it is one for a protected resource on
    /// another origin, which an earlier `loft` could keep: it is no sign-in for the address and
    /// is neither refreshed nor sent, whatever refresh it records, and its error, unlike that of
    /// a missing one, names that resource. Or it records a refresh in a member that is not a
    /// string: a refresh that cannot be repeated may have rotated the refresh token, so none is
    /// sent, and it reads as lapsed.
    pub(super) fn find(entries: &Map<String, Value>, address: &str) -> Result<Self, RemoteError> {
        let none = || RemoteError::NoSignIn {
            address: address.to_owned(),
        };
        // The derived reader would also read a list of the members' values, in their order, and
        // a refresh could keep nothing it issued in a list.
        let entry = entries.get(address).filter(|entry| entry.is_object());
        let entry = entry.ok_or_else(none)?;
        let sign_in = SignIn::deserialize(entry).map_err(|_| none())?;
        // A resource that reads as no address lies on no origin, so it is not on another one.
        let origin = network::origin(&sign_in.resource).ok_or_else(none)?;
        if network::origin(address).as_ref() != Some(&origin) {
            return Err(RemoteError::SignInElsewhere {
                address: address.to_owned(),
                resource: sign_in.resource,
            });
        }
        let refreshing = match entry.get(REFRESHING) {
            None => None,
            Some(Value::String(resource)) => Some(resource.clone()),
            Some(_) => {
                return Err(RemoteError::Lapsed {
                    address: address.to_owned(),
                    error: UNREADABLE_RECORD.to_owned(),
                });
            }
        };
        Ok(Self {
            sign_in,
            refreshing,
            records: entry.get(RESOURCES).is_some(),
        })
    }

    /// The identifier of the protected resource that an access token is wanted for: the
    /// remote's own, where `resource` is none or names it, and otherwise a companion resource
    /// that the sign-in lists on the origin of `address`.
    ///
    /// # Errors
    ///
    /// The sign-in does not list `resource` on that origin, so it does not cover it.
    pub(super) fn wanted(
        &self,
        address: &str,
        resource: Option<&str>,
    ) -> Result<String, RemoteError> {
        let own = &self.sign_in.resource;
        let Some(resource) = resource.filter(|resource| resource != own) else {
            return Ok(own.clone());
        };
        if self.companion(resource).is_some() && network::same_origin(resource, address) {
            Ok(resource.to_owned())
        } else {
            Err(RemoteError::NotCovered {
                address: address.to_owned(),
                resource: resource.to_owned(),
            })
        }
    }

    /// The companion resource of the identifier `resource` that the sign-in lists, if it lists
    /// one.
    fn companion(&self, resource: &str) -> Option<&Covered> {
        let mut listed = self.sign_in.resources.iter();
        listed.find(|covered| covered.resource == resource)
    }

    /// The access token kept for `resource`, where it expires later than five minutes after
    /// `now`, whether or not a refresh is recorded.
    pub(super) fn fresh(&self, resource: &str, now: u64) -> Option<String> {
        let (token, expires_at) = if resource == self.sign_in.resource {
            (&self.sign_in.access_token, &self.sign_in.expires_at)
        } else {
            let covered = self.companion(resource)?;
            (covered.access_token.as_ref()?, covered.expires_at.as_ref()?)
        };
        (!expiring(expires_at, now)).then(|| token.clone())
    }

    /// The access token kept for `resource`, where the sign-in records no refresh and it expires
    /// later than five minutes after `now`.
    pub(super) fn live(&self, resource: &str, now: u64) -> Option<String> {
        if self.refreshing.is_some() {
            return None;
        }
        self.fresh(resource, now)
    }
}

/// Records in `entry`, a sign-in as the file keeps it, that a refresh for `resource` is about to
/// be sent.
pub(super) fn record(entry: &mut Value, resource: &str) {
    if let Some(members) = entry.as_object_mut() {
        members.insert(REFRESHING.to_owned(), Value::from(resource));
    }
}

/// Removes from `entry` the record of a refresh.
pub(super) fn unrecord(entry: &mut Value) {
    if let Some(members) = entry.as_object_mut() {
        members.remove(REFRESHING);
    }
}

/// Keeps in `entry` what a refresh for `resource` issued: the access token and its expiry where
/// the entry keeps that resource's, at the top for `own`, the remote's protected resource, and in
/// its object of `resources` for a companion resource; the refresh token, for the whole sign-in;
/// and no record. Every other member is kept as it was.
pub(super) fn keep_issued(entry: &mut Value, own: &str, resource: &str, issued: &Issued) {
    let token = |members: &mut Map<String, Value>| {
        members.insert("accessToken".to_owned(), Value::from(&*issued.access_token));
        members.insert("expiresAt".to_owned(), Value::from(&*issued.expires_at));
    };
    let Some(members) = entry.as_object_mut() else {
        return;
    };
    if resource == own {
        token(members);
    } else if let Some(listed) = members
        .get_mut(RESOURCES)
        .and_then(Value::as_array_mut)
        .and_then(|listed| listed.iter_mut().find(|covered| names(covered, resource)))
        .and_then(Value::as_object_mut)
    {
        token(listed);
    }
    members.insert(
        "refreshToken".to_owned(),
        Value::from(&*issued.refresh_token),
    );
    members.remove(REFRESHING);
}

/// Whether `covered`, an object of `resources`, is the companion resource `resource`.
fn names(covered: &Value, resource: &str) -> bool {
    covered.get("resource").and_then(Value::as_str) == Some(resource)
}

#[cfg(test)]
mod tests {
    use serde_json::{Map, Value, json};

    use super::{Kept, keep_issued};
    use crate::remote::RemoteError;
    use crate::remote::signin::Issued;

    const ADDRESS: &str = "https://remote.example/documents/one";
    const OWN: &str = "https://remote.example/documents";
    const NOTES: &str = "https://remote.example/notes";
    const INBOX: &str = "https://remote.example/mail/inbox?view=all";
    /// Long before any token below expires.
    const NOW: u64 = 1_800_000_000;
    const LATER: &str = "2100-01-01T00:00:00Z";

    /// A sign-in in the shape of an earlier `loft`: the six members alone.
    fn earlier() -> Value {
        json!({
            "tokenEndpoint": "https://remote.example/auth/token",
            "resource": OWN,
            "clientId": "client-1",
            "accessToken": "access-1",
            "expiresAt": LATER,
            "refreshToken": "refresh-1",
        })
    }

    /// `earlier` with `member` set to `value`.
    fn with(member: &str, value: Value) -> Value {
        let mut entry = earlier();
        entry[member] = value;
        entry
    }

    /// What `entry`, kept for the address, reads as.
    fn found(entry: Value) -> Result<Kept, RemoteError> {
        Kept::find(&Map::from_iter([(ADDRESS.to_owned(), entry)]), ADDRESS)
    }

    fn not_covered(resource: &str) -> Result<String, RemoteError> {
        Err(RemoteError::NotCovered {
            address: ADDRESS.to_owned(),
            resource: resource.to_owned(),
        })
    }

    #[test]
    fn a_sign_in_in_the_shape_of_an_earlier_loft_covers_its_resource_alone() {
        let held = found(earlier()).unwrap();
        assert_eq!(held.wanted(ADDRESS, None), Ok(OWN.to_owned()));
        assert_eq!(held.wanted(ADDRESS, Some(OWN)), Ok(OWN.to_owned()));
        assert_eq!(held.wanted(ADDRESS, Some(NOTES)), not_covered(NOTES));
        assert_eq!(held.live(OWN, NOW).as_deref(), Some("access-1"));
        assert_eq!(held.refreshing, None);
        assert!(
            !held.records,
            "a sign-in without `resources` records no refresh"
        );
    }

    #[test]
    fn a_sign_in_whose_companion_resources_do_not_read_still_serves_its_own_resource() {
        let issuer = "https://remote.example/auth";
        let cases = [
            (json!(3), None, json!("x")),
            (json!(null), None, json!([{ "resource": NOTES }])),
            (
                json!(issuer),
                Some(issuer),
                json!([{ "resource": NOTES, "kind": 3 }]),
            ),
        ];
        for (kept, read, resources) in cases {
            let mut entry = with("issuer", kept);
            entry["resources"] = resources.clone();
            let held = found(entry).unwrap();
            assert_eq!(held.sign_in.issuer.as_deref(), read, "{resources}");
            assert!(held.sign_in.resources.is_empty(), "{resources}");
            assert_eq!(
                held.wanted(ADDRESS, None),
                Ok(OWN.to_owned()),
                "{resources}"
            );
            assert_eq!(held.wanted(ADDRESS, Some(NOTES)), not_covered(NOTES));
            assert_eq!(held.live(OWN, NOW).as_deref(), Some("access-1"));
        }
    }

    #[test]
    fn a_companion_resource_listed_on_another_origin_than_the_address_is_not_covered() {
        let others = [
            "https://elsewhere.example/notes",
            "https://remote.example:8443/notes",
        ];
        let listed = [NOTES, others[0], others[1]].map(|resource| {
            json!({ "resource": resource, "kind": resource, "accessToken": resource,
                "expiresAt": LATER })
        });
        let held = found(with("resources", json!(listed))).unwrap();
        assert_eq!(held.wanted(ADDRESS, Some(NOTES)), Ok(NOTES.to_owned()));
        assert_eq!(held.live(NOTES, NOW).as_deref(), Some(NOTES));
        for other in others {
            assert_eq!(held.wanted(ADDRESS, Some(other)), not_covered(other));
        }
    }

    #[test]
    fn a_refresh_replaces_only_the_members_it_writes() {
        let issued = Issued {
            access_token: "access-9".to_owned(),
            expires_at: "2100-01-02T00:00:00Z".to_owned(),
            refresh_token: "refresh-9".to_owned(),
        };
        // The remote's own token, in a sign-in without `resources` that a later `loft` added to.
        let entry = with("later", json!("kept by a later loft"));
        let mut refreshed = entry.clone();
        keep_issued(&mut refreshed, OWN, OWN, &issued);
        let mut expected = entry;
        expected["accessToken"] = json!("access-9");
        expected["expiresAt"] = json!("2100-01-02T00:00:00Z");
        expected["refreshToken"] = json!("refresh-9");
        assert_eq!(refreshed, expected, "no member is added or dropped");
        // A companion resource's token, which a record named.
        let mut entry = with("issuer", json!("https://remote.example/auth"));
        entry["resources"] = json!([
            { "resource": NOTES, "kind": "notes" },
            { "resource": INBOX, "kind": "inbox", "accessToken": "access-3", "expiresAt": LATER },
        ]);
        entry["refreshing"] = json!(NOTES);
        let mut refreshed = entry.clone();
        keep_issued(&mut refreshed, OWN, NOTES, &issued);
        let mut expected = entry;
        expected.as_object_mut().unwrap().remove("refreshing");
        expected["refreshToken"] = json!("refresh-9");
        expected["resources"][0]["accessToken"] = json!("access-9");
        expected["resources"][0]["expiresAt"] = json!("2100-01-02T00:00:00Z");
        assert_eq!(
            refreshed, expected,
            "the remote's own token and the inbox's are kept"
        );
    }

    #[test]
    fn a_sign_in_that_holds_resources_records_each_refresh_even_where_they_do_not_read() {
        assert!(!found(earlier()).unwrap().records);
        let listed = json!([{ "resource": NOTES, "kind": "notes" }]);
        for resources in [json!([]), json!("x"), json!(null), listed] {
            let held = found(with("resources", resources.clone())).unwrap();
            assert!(held.records, "{resources}");
        }
    }

    #[test]
    fn a_record_that_is_not_a_string_reads_as_a_lapsed_sign_in() {
        let lapsed = Err(RemoteError::Lapsed {
            address: ADDRESS.to_owned(),
            error: "a recorded refresh does not read".to_owned(),
        });
        for record in [
            json!({ "resource": NOTES }),
            json!(3),
            json!(null),
            json!([NOTES]),
        ] {
            let read = found(with("refreshing", record.clone())).map(|held| held.refreshing);
            assert_eq!(read, lapsed, "{record}");
        }
        let held = found(with("refreshing", json!(NOTES))).unwrap();
        assert_eq!(held.refreshing.as_deref(), Some(NOTES));
        // A recorded refresh is repeated before a token is given, however long it has left.
        assert_eq!(held.fresh(OWN, NOW).as_deref(), Some("access-1"));
        assert_eq!(held.live(OWN, NOW), None);
        assert_eq!(found(earlier()).unwrap().refreshing, None);
    }
}
