//! `loft yard page` (the Give the Yard page's address Feature): the address of the branch's Yard
//! Log on the page, with a line on what the page shows, which an agent host with no command of its
//! own for the page relays; or that no page server serves it. It writes no file of the tree.

use std::path::Path;

use serde_json::Value;
use yardlog_store::Place;
use yardlog_store::git::YardGit;

use super::args::PageArgs;
use super::branch::Branch;
use super::failure::Failure;
use super::page;
use super::working_tree::{Follows, target};
use crate::finish::Finished;
use crate::json;
use crate::out::outln;

const SCHEMA: &str = "loft-yard-page/1";

pub fn run(root: &Path, args: &PageArgs) -> Finished {
    let as_json = args.output.json();
    match document(root, args) {
        Ok(fields) => {
            let is_available = fields["page"]["available"] == true;
            let outcome = if is_available { "page" } else { "unavailable" };
            if as_json {
                json::print(&json::result(SCHEMA, outcome, fields, Vec::new()));
            } else {
                for line in words(&fields) {
                    outln!("{line}");
                }
            }
            if is_available {
                Finished::success(outcome)
            } else {
                Finished::failure(outcome)
            }
        }
        Err(failure) => failure.finish(SCHEMA, as_json),
    }
}

/// Where the command's tree is, with the page's address for its branch.
fn document(root: &Path, args: &PageArgs) -> Result<Value, Failure> {
    let dir = target(root, args.tree.as_deref(), None, Follows::Directory)?;
    let git = YardGit::default();
    let place = Place::locate(&git, &dir)?;
    let branch = Branch::at(git, place, args.beyond.clone())?;
    let mut fields = branch.located();
    fields["page"] = page::address(
        &branch.place().tree,
        branch.store.branch(),
        branch.beyond.port,
    );
    Ok(fields)
}

/// The words for a person: the address and what the page shows, or that no page server serves it.
fn words(fields: &Value) -> Vec<String> {
    match fields["page"]["url"].as_str() {
        Some(url) => vec![
            format!(
                "The Yard page for {} shows its Yard Log in full: {url}",
                fields["branch"].as_str().unwrap_or_default()
            ),
            "Open it in your browser.".to_owned(),
        ],
        None => {
            vec!["No page server serves the Yard page; `loft yard serve` starts one.".to_owned()]
        }
    }
}
