//! What a remote simulator holds and how it answers: its scenario, built from the contract's
//! answers and the failures a test scripts.

use std::collections::BTreeMap;
use std::time::Duration;

use serde_json::Value;

use super::{Companion, StoredState};

/// How the person answers an authorization request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Consent {
    Grant,
    Deny,
}

#[derive(Clone, Debug)]
pub(super) enum Mode {
    Accepting,
    Refusing,
    Failing { status: u16, code: Option<String> },
    Silent,
}

/// To whom a remote serves its document, as its head names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visibility {
    /// Anyone: the head and the published states are answered with no token.
    Public,
    /// Those the remote admits alone.
    Private,
}

impl Visibility {
    /// The name the head gives it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
        }
    }
}

/// What the two metadata documents say, as the contract states them or as a misconfigured
/// server serves them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Metadata {
    /// As the contract states them.
    Faithful,
    /// A challenge without `scope`, and the protected resource metadata listing these scopes.
    Unscoped(Vec<String>),
    /// Protected resource metadata whose `resource` is not its own.
    MisnamedResource,
    /// Authorization server metadata whose `issuer` is not the one the resource metadata names.
    MisnamedIssuer,
    /// Authorization server metadata without `registration_endpoint`.
    Unregistering,
    /// Authorization server metadata whose `code_challenge_methods_supported` is `plain` alone.
    Plain,
    /// Authorization server metadata whose `authorization_endpoint` is `http` off the loopback
    /// interface.
    Unencrypted,
    /// Protected resource metadata that the challenge names on another origin, which a second
    /// port of the loopback interface stands for, naming the protected resource there. The
    /// simulator's own origin answers `not_found` where it would otherwise lie.
    OnAnotherOrigin,
    /// Protected resource metadata that the challenge names on the simulator's own origin, at the
    /// well-known path followed by `@`, the host and port of a second port of the loopback
    /// interface, which stands for another origin, and the protected resource's path. Its
    /// `resource` is its own address with the well-known path taken out, as RFC 9728 derives it:
    /// the simulator's origin is that address's user information, and the second port's host and
    /// port its host and port, so that it lies on the second port's origin. The second port
    /// serves only the metadata of a companion resource its scenario places there, and answers
    /// `not_found` to every other request; the metadata's usual address still serves it
    /// faithfully.
    ResourceOnAnotherOrigin,
}

/// The skill package a scenario serves: its manifest's `name` and `version`, and its bytes.
#[derive(Clone, Debug)]
pub(super) struct Skill {
    pub(super) name: String,
    pub(super) version: String,
    pub(super) package: Vec<u8>,
}

/// An answer a scenario misstates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Misstating {
    /// An accepted publish's lock.
    Lock,
    /// An accepted import's head.
    ImportHead,
    /// The skill package, whose bytes are not the ones its manifest names.
    Package,
}

/// A call a scenario can forbid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Call {
    Create,
    Import,
}

/// The document a scenario holds: its name, its published states and its drafts.
#[derive(Clone, Debug, Default)]
pub(super) struct Held {
    pub(super) name: String,
    pub(super) states: Vec<StoredState>,
    pub(super) drafts: BTreeMap<String, BTreeMap<String, String>>,
}

/// What a remote simulator holds and how it answers.
#[derive(Clone, Debug)]
pub struct RemoteScenario {
    pub(super) key: String,
    pub(super) mode: Mode,
    pub(super) retry_after: Option<String>,
    pub(super) held: Option<Held>,
    /// The calls that every token is forbidden.
    pub(super) forbidden: Vec<Call>,
    pub(super) closed: Option<String>,
    pub(super) import_limit: Option<usize>,
    pub(super) state_limit: Option<usize>,
    pub(super) consent: Consent,
    pub(super) expires_in: u64,
    /// How long a rotated refresh token is answered again for the resource its rotation named.
    pub(super) grace: Duration,
    pub(super) lapsed: bool,
    pub(super) scope: String,
    pub(super) publish_from: u64,
    pub(super) metadata: Metadata,
    /// The OAuth error every registration is refused with.
    pub(super) refused_registration: Option<String>,
    /// The OAuth error every code exchange that names a single resource, one the scenario serves,
    /// is refused with.
    pub(super) refused_exchange: Option<String>,
    /// Whether the next refresh's answer is lost after the token rotates.
    pub(super) losing_refresh: bool,
    /// How long each refresh waits before it is answered.
    pub(super) refresh_delay: Duration,
    /// The paths every publish is refused as `invalid_request` naming.
    pub(super) invalid_publish: Option<Vec<String>>,
    /// The answers of accepted calls that their states do not have.
    pub(super) misstating: Vec<Misstating>,
    /// The skill package the scenario serves, if any.
    pub(super) skill: Option<Skill>,
    /// The companion resources that the document's protected resource metadata lists, in order.
    pub(super) companions: Vec<Companion>,
    /// The member that lists the companion resources, as the scenario scripts it.
    pub(super) listing: Option<Value>,
    /// To whom the remote serves its document, where the scenario names it.
    pub(super) visibility: Option<Visibility>,
}

impl RemoteScenario {
    /// A remote that answers as the contract states, holding no document under `key`.
    #[must_use]
    pub fn accepting(key: &str) -> Self {
        Self {
            key: key.to_owned(),
            mode: Mode::Accepting,
            retry_after: None,
            held: None,
            forbidden: Vec::new(),
            closed: None,
            import_limit: None,
            state_limit: None,
            consent: Consent::Grant,
            expires_in: 86_400,
            grace: Duration::from_secs(120),
            lapsed: false,
            scope: "documents".to_owned(),
            publish_from: 1_800_000_000,
            metadata: Metadata::Faithful,
            refused_registration: None,
            refused_exchange: None,
            losing_refresh: false,
            refresh_delay: Duration::ZERO,
            invalid_publish: None,
            misstating: Vec::new(),
            skill: None,
            companions: Vec::new(),
            listing: None,
            visibility: None,
        }
    }

    /// A remote that answers every request with `status`, and with a problem document of `code`
    /// where it names one.
    #[must_use]
    pub fn failing(key: &str, status: u16, code: Option<&str>) -> Self {
        Self {
            mode: Mode::Failing {
                status,
                code: code.map(str::to_owned),
            },
            ..Self::accepting(key)
        }
    }

    /// A remote that accepts each connection and never answers.
    #[must_use]
    pub fn silent(key: &str) -> Self {
        Self {
            mode: Mode::Silent,
            ..Self::accepting(key)
        }
    }

    /// A remote whose port refuses every connection.
    #[must_use]
    pub fn refusing(key: &str) -> Self {
        Self {
            mode: Mode::Refusing,
            ..Self::accepting(key)
        }
    }

    /// The remote, naming `visibility` in its head, and serving its head and published states
    /// with no token where it is public.
    #[must_use]
    pub fn visibility(mut self, visibility: Visibility) -> Self {
        self.visibility = Some(visibility);
        self
    }

    /// Holds the document under the scenario's key, named `name`, with `states` published.
    #[must_use]
    pub fn holding(mut self, name: &str, states: Vec<StoredState>) -> Self {
        let held = self.held.get_or_insert_with(Held::default);
        name.clone_into(&mut held.name);
        held.states = states;
        self
    }

    /// Holds a draft under `branch`, of `files` by their paths from the top of the tree.
    #[must_use]
    pub fn draft(mut self, branch: &str, files: BTreeMap<String, String>) -> Self {
        let held = self.held.get_or_insert_with(Held::default);
        held.drafts.insert(branch.to_owned(), files);
        self
    }

    /// Sends `Retry-After: value` with each failing answer.
    #[must_use]
    pub fn retry_after(mut self, value: &str) -> Self {
        self.retry_after = Some(value.to_owned());
        self
    }

    /// Refuses every create as `forbidden`.
    #[must_use]
    pub fn forbidding_create(mut self) -> Self {
        self.forbidden.push(Call::Create);
        self
    }

    /// Refuses every import as `forbidden`.
    #[must_use]
    pub fn forbidding_import(mut self) -> Self {
        self.forbidden.push(Call::Import);
        self
    }

    /// Refuses every publish as a `conflict` at the head, with `detail` as its reason.
    #[must_use]
    pub fn closed(mut self, detail: &str) -> Self {
        self.closed = Some(detail.to_owned());
        self
    }

    /// Refuses an import whose body is longer than `bytes` as `payload_too_large`.
    #[must_use]
    pub fn import_limit(mut self, bytes: usize) -> Self {
        self.import_limit = Some(bytes);
        self
    }

    /// Refuses an import that holds a state whose files are longer than `bytes` together, as
    /// `payload_too_large`.
    #[must_use]
    pub fn state_limit(mut self, bytes: usize) -> Self {
        self.state_limit = Some(bytes);
        self
    }

    /// The person refuses each authorization request.
    #[must_use]
    pub fn refusing_consent(mut self) -> Self {
        self.consent = Consent::Deny;
        self
    }

    /// Issues access tokens that expire in `seconds`.
    #[must_use]
    pub fn expiring_in(mut self, seconds: u64) -> Self {
        self.expires_in = seconds;
        self
    }

    /// Answers a rotated refresh token presented again for the resource its rotation named
    /// within `grace` of its rotation, in place of 120 seconds.
    #[must_use]
    pub fn grace(mut self, grace: Duration) -> Self {
        self.grace = grace;
        self
    }

    /// Answers no rotated refresh token again: one that its client presents again for a resource
    /// the scenario serves is refused `invalid_grant`, and revokes its sign-in.
    #[must_use]
    pub fn without_grace(self) -> Self {
        self.grace(Duration::ZERO)
    }

    /// Refuses `invalid_grant` every refresh that names a single resource, one the scenario
    /// serves, as a sign-in that has lapsed.
    #[must_use]
    pub fn lapsing(mut self) -> Self {
        self.lapsed = true;
        self
    }

    /// Dates the remote's publishes from `seconds` after the Unix epoch, a second apart.
    #[must_use]
    pub fn publishing_from(mut self, seconds: u64) -> Self {
        self.publish_from = seconds;
        self
    }

    /// Serves the metadata documents as `metadata` says.
    #[must_use]
    pub fn metadata(mut self, metadata: Metadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Refuses every registration with the OAuth error `error`.
    #[must_use]
    pub fn refusing_registration(mut self, error: &str) -> Self {
        self.refused_registration = Some(error.to_owned());
        self
    }

    /// Refuses with the OAuth error `error` every code exchange that names a single resource, one
    /// the scenario serves.
    #[must_use]
    pub fn refusing_exchange(mut self, error: &str) -> Self {
        self.refused_exchange = Some(error.to_owned());
        self
    }

    /// Loses the answer of the next refresh that rotates a token: the token rotates, then the
    /// connection closes without an answer.
    #[must_use]
    pub fn losing_a_refresh_answer(mut self) -> Self {
        self.losing_refresh = true;
        self
    }

    /// Answers each refresh only after `delay`.
    #[must_use]
    pub fn delaying_refreshes(mut self, delay: Duration) -> Self {
        self.refresh_delay = delay;
        self
    }

    /// Refuses every publish as `invalid_request` naming `paths`.
    #[must_use]
    pub fn refusing_publish_as_invalid(mut self, paths: &[&str]) -> Self {
        self.invalid_publish = Some(paths.iter().map(|path| (*path).to_owned()).collect());
        self
    }

    /// Answers each accepted publish with a lock whose state root its files do not have.
    #[must_use]
    pub fn misstating_its_lock(mut self) -> Self {
        self.misstating.push(Misstating::Lock);
        self
    }

    /// Answers each accepted import with a head other than its last state.
    #[must_use]
    pub fn misstating_its_import_head(mut self) -> Self {
        self.misstating.push(Misstating::ImportHead);
        self
    }

    /// Serves a skill package of `package`'s bytes, its manifest naming it `name` at `version`.
    #[must_use]
    pub fn skill(mut self, name: &str, version: &str, package: Vec<u8>) -> Self {
        self.skill = Some(Skill {
            name: name.to_owned(),
            version: version.to_owned(),
            package,
        });
        self
    }

    /// Serves other bytes than the package its manifest names.
    #[must_use]
    pub fn tampering(mut self) -> Self {
        self.misstating.push(Misstating::Package);
        self
    }
}
