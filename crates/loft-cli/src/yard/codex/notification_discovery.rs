//! Extract only authoritative submission metadata; never copy a question or answer into a nudge.

use std::path::Path;

use serde_json::Value;

use super::notification_state::{Resolution, Route};

pub struct Snapshot {
    pub log: String,
    pub tree: String,
    pub branch: String,
    pub closed: bool,
    pub own_read: Option<u64>,
    pub resolved: Vec<Resolution>,
    pub withdrawn: Vec<String>,
}

impl Snapshot {
    pub fn read(doc: &Value, route: &Route) -> Option<Self> {
        if doc["schema"] != "loft-yard-read/3" || doc["outcome"] != "read" {
            return None;
        }
        if doc["leftOut"].as_u64().is_some_and(|count| count > 0) {
            return None;
        }
        let log = doc["yardLog"]["key"].as_str()?.to_owned();
        let tree = doc["tree"].as_str()?.to_owned();
        let branch = doc["branch"].as_str()?.to_owned();
        let directory = Path::new(&route.directory);
        let root = Path::new(&tree);
        if !directory.starts_with(root)
            && !directory
                .canonicalize()
                .ok()
                .zip(root.canonicalize().ok())
                .is_some_and(|(dir, root)| dir.starts_with(root))
        {
            return None;
        }
        let mut snapshot = Self {
            log,
            tree,
            branch,
            closed: doc["yardLog"]["closedBy"].as_str()? != "open",
            own_read: doc["ownLastRead"].as_u64(),
            resolved: vec![],
            withdrawn: vec![],
        };
        for sent in doc["sent"].as_array()? {
            if !route.caller.owns(&sent["agent"]) {
                continue;
            }
            let Some(submission) = sent["key"].as_str() else {
                continue;
            };
            let Some(revision) = sent["reached"].as_u64() else {
                continue;
            };
            let resolution = Resolution {
                log: snapshot.log.clone(),
                tree: snapshot.tree.clone(),
                branch: snapshot.branch.clone(),
                submission: submission.into(),
                revision,
                thread: route.thread.clone(),
            };
            match sent["state"].as_str() {
                Some("answered") => snapshot.resolved.push(resolution),
                Some("withdrawn") => snapshot.withdrawn.push(resolution.marker()),
                _ => {}
            }
        }
        Some(snapshot)
    }

    pub fn contains(&self, resolution: &Resolution) -> bool {
        !self.closed
            && self.log == resolution.log
            && self.tree == resolution.tree
            && self.branch == resolution.branch
            && self
                .resolved
                .iter()
                .any(|item| item.submission == resolution.submission)
    }
}

#[cfg(test)]
#[path = "notification_discovery_tests.rs"]
mod tests;
