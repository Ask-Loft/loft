//! What a remote simulator holds and how it answers: its scenario, built from the contract's
//! answers and the failures a test scripts.

use std::collections::BTreeMap;
use std::time::Duration;

use super::StoredState;

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
    /// How long a rotated refresh token is answered again.
    pub(super) grace: Duration,
    pub(super) lapsed: bool,
    pub(super) scope: String,
    pub(super) publish_from: u64,
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

    /// Answers a rotated refresh token presented again for `grace` after its rotation, in
    /// place of 120 seconds.
    #[must_use]
    pub fn grace(mut self, grace: Duration) -> Self {
        self.grace = grace;
        self
    }

    /// Refuses a rotated refresh token presented again, and revokes its sign-in.
    #[must_use]
    pub fn without_grace(self) -> Self {
        self.grace(Duration::ZERO)
    }

    /// Refuses every refresh, as a sign-in that has lapsed.
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
}
