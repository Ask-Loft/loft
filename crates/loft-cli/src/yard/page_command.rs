//! `loft yard page` (the Give the Yard page's address Feature): the address of the branch's Yard
//! Log on the page, with a line on what the page shows, which an agent host with no command of its
//! own for the page relays; or that no page server serves it. It writes no file of the tree.

use std::path::Path;

use serde_json::Value;

use super::args::PageArgs;
use super::branch::Branch;
use super::failure::Failure;
use super::working_tree::{Follows, target};
use crate::finish::Finished;
use crate::json;
use crate::out::outln;

const SCHEMA: &str = "loft-yard-page/1";

pub fn run(root: &Path, args: &PageArgs) -> Finished {
    let as_json = args.output.json();
    let fields = match document(root, args) {
        Ok(fields) => fields,
        Err(failure) => return failure.finish(SCHEMA, as_json),
    };
    let url = fields["page"]["url"].as_str().map(ToOwned::to_owned);
    let (outcome, finish): (_, fn(&'static str) -> Finished) = match url {
        Some(_) => ("page", Finished::success),
        None => ("unavailable", Finished::failure),
    };
    if as_json {
        json::print(&json::result(SCHEMA, outcome, fields, Vec::new()));
    } else {
        for line in words(
            fields["branch"].as_str().unwrap_or_default(),
            url.as_deref(),
        ) {
            outln!("{line}");
        }
    }
    finish(outcome)
}

/// Where the command's tree is, with the page's address for its branch.
fn document(root: &Path, args: &PageArgs) -> Result<Value, Failure> {
    let dir = target(root, args.tree.as_deref(), None, Follows::Directory)?;
    let branch = Branch::locate(&dir, args.beyond.clone())?;
    let mut fields = branch.located();
    fields["page"] = branch.page();
    Ok(fields)
}

/// The words for a person: the address and what the page shows, or that no page server serves it.
fn words(branch: &str, url: Option<&str>) -> Vec<String> {
    match url {
        Some(url) => vec![
            format!("The Yard page for {branch} shows its Yard Log in full: {url}"),
            "Open it in your browser.".to_owned(),
        ],
        None => {
            vec!["No page server serves the Yard page; `loft yard serve` starts one.".to_owned()]
        }
    }
}
