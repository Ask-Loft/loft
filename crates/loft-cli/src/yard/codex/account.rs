//! Only account provenance is retained; credentials and personal account details are discarded.

use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Account {
    ApiKey,
    Chatgpt,
    #[default]
    Unknown,
}

impl Account {
    pub fn source(value: &Value) -> Self {
        match value.as_str() {
            Some("apiKey") => Self::ApiKey,
            Some("chatgpt") => Self::Chatgpt,
            _ => Self::Unknown,
        }
    }

    pub fn updated(value: &Value) -> Self {
        match value.as_str() {
            Some("apikey") => Self::ApiKey,
            Some("chatgpt" | "chatgptAuthTokens") => Self::Chatgpt,
            _ => Self::Unknown,
        }
    }
}

pub struct Provenance {
    kind: Account,
    updated: bool,
    eligible: bool,
}

impl Default for Provenance {
    fn default() -> Self {
        Self {
            kind: Account::Unknown,
            updated: false,
            eligible: true,
        }
    }
}

impl Provenance {
    pub fn initial(&mut self, response: Option<&Value>) {
        let required = response.and_then(|v| v["requiresOpenaiAuth"].as_bool());
        self.eligible = required != Some(false);
        if !self.eligible {
            self.kind = Account::Unknown;
        } else if !self.updated {
            self.kind = response
                .filter(|_| required == Some(true))
                .map_or(Account::Unknown, |v| Account::source(&v["account"]["type"]));
        }
    }

    pub fn update(&mut self, mode: &Value) {
        self.updated = true;
        self.kind = if self.eligible {
            Account::updated(mode)
        } else {
            Account::Unknown
        };
    }

    pub const fn kind(&self) -> Account {
        self.kind
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_provider_without_openai_auth_does_not_borrow_the_accounts_api_key_classification() {
        let mut source = Provenance::default();
        source.initial(Some(
            &json!({"requiresOpenaiAuth":false,"account":{"type":"apiKey"}}),
        ));
        source.update(&json!("apikey"));
        assert_eq!(source.kind(), Account::Unknown);
    }

    #[test]
    fn an_account_update_received_during_lookup_wins_over_its_older_snapshot() {
        for (mode, expected) in [
            (json!("chatgpt"), Account::Chatgpt),
            (Value::Null, Account::Unknown),
        ] {
            let mut source = Provenance::default();
            source.update(&mode);
            source.initial(Some(
                &json!({"requiresOpenaiAuth":true,"account":{"type":"apiKey"}}),
            ));
            assert_eq!(source.kind(), expected);
        }
    }
}
