//! `loft yard continue` (the Record a continuation Feature): records once, in
//! `~/.yard/continuations.json`, that a session continues one that ended, as after `/clear` or a
//! resume, so that every later command counts it as the first session of its chain. It needs no
//! repository and writes nothing under `.yard/`.

use serde_json::json;
use yardlog_store::home::Home;

use super::args::ContinueArgs;
use super::caller::record_continuation;
use super::failure::Failure;
use crate::finish::Finished;
use crate::json;
use crate::out::outln;

const SCHEMA: &str = "loft-yard-continue/1";
const OUTCOME: &str = "recorded";

pub fn run(args: &ContinueArgs) -> Finished {
    let as_json = args.output.json();
    let recorded = Home::from_env()
        .ok_or_else(Failure::no_home)
        .and_then(|home| record_continuation(&home, &args.session, &args.from));
    match recorded {
        Ok(changed) => {
            let fields = json!({
                "session": args.session,
                "continues": args.from,
                "changed": changed,
            });
            if as_json {
                json::print(&json::result(SCHEMA, OUTCOME, fields, Vec::new()));
            } else {
                outln!("session {} continues {}", args.session, args.from);
            }
            Finished::success(OUTCOME)
        }
        Err(failure) => failure.finish(SCHEMA, as_json),
    }
}
