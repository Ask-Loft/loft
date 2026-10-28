//! `loft yard continue` (the Record a continuation Feature): records once, in
//! `~/.yard/continuations.json`, that a session continues one that ended, as after `/clear` or a
//! resume, so that every later command counts it as the first session of its chain. It needs no
//! repository and writes nothing under `.yard/`.

use serde_json::json;
use yardlog_store::home::Home;

use super::args::ContinueArgs;
use super::caller::record_continuation;
use super::failure::{self, Failure};
use crate::finish::Finished;

const SCHEMA: &str = "loft-yard-continue/1";
const OUTCOME: &str = "recorded";

pub fn run(args: &ContinueArgs) -> Finished {
    let recorded = Home::from_env()
        .ok_or_else(Failure::no_home)
        .and_then(|home| record_continuation(&home, &args.host, &args.session, &args.from))
        .map(|changed| {
            let fields = json!({
                "session": args.session,
                "continues": args.from,
                "changed": changed,
            });
            (OUTCOME, fields)
        });
    failure::end(SCHEMA, args.output.json(), recorded, |_, _| {
        format!("session {} continues {}", args.session, args.from)
    })
}
