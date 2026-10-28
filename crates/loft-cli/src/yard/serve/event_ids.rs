//! Durable receipts for an event's original payload, before receiver annotations are added.

use std::collections::BTreeMap;

use serde_json::Value;
use sha2::{Digest, Sha256};

const DIGEST: &str = "eventDigest";

#[derive(Clone, Debug)]
pub struct Receipt {
    id: String,
    digest: String,
}

impl Receipt {
    pub fn read(posted: &Value) -> Result<Option<Self>, String> {
        let Some(id) = posted.get("eventId") else {
            return Ok(None);
        };
        let id = id
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or("eventId must be a nonempty string")?;
        Ok(Some(Self {
            id: id.to_owned(),
            digest: format!("{:x}", Sha256::digest(posted.to_string().as_bytes())),
        }))
    }

    pub fn annotate(&self, line: &mut Value) {
        line[DIGEST] = Value::String(self.digest.clone());
    }

    /// The receipt a kept line carries: none for a line kept without its digest.
    pub fn replay(line: &Value) -> Result<Option<Self>, String> {
        let Some(mut receipt) = Self::read(line)? else {
            return Ok(None);
        };
        let Some(digest) = line.get(DIGEST).and_then(Value::as_str) else {
            return Ok(None);
        };
        digest.clone_into(&mut receipt.digest);
        Ok(Some(receipt))
    }
}

#[derive(Default)]
pub struct Receipts(BTreeMap<String, String>);

impl Receipts {
    pub fn contains(&self, receipt: Option<&Receipt>) -> Result<bool, String> {
        let Some(receipt) = receipt else {
            return Ok(false);
        };
        let Some(digest) = self.0.get(&receipt.id) else {
            return Ok(false);
        };
        if digest != &receipt.digest {
            return Err("eventId was already used with a different payload".to_owned());
        }
        Ok(true)
    }

    pub fn keep(&mut self, receipt: Option<&Receipt>) {
        if let Some(receipt) = receipt {
            self.0.insert(receipt.id.clone(), receipt.digest.clone());
        }
    }
}
