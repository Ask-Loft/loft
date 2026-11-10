//! What a session's agents consulted, as the page server counts it from its events (the Feature
//! "Count what the agents consulted"): a tool call's start adds its classes and its `loft` reads,
//! its end readies its result for the agent's next request, and a model request attributes the
//! growth of its prompt to the calls that ended before it. Only a session whose adapter's
//! descriptor declares `consults` is counted.

use yardlog::adapter::Descriptor;
use yardlog::consult::{Call, Class, Consultation, reads_spec};

use crate::yard::event::{Event, strings};

/// The classes `event` names in its field `name`, each it knows.
fn classes(event: &Event, name: &str) -> Vec<Class> {
    strings(&event.posted[name])
        .iter()
        .filter_map(|class| Class::named(class))
        .collect()
}

/// Counts `event` into `consultation` under the session's descriptor `adapter`, `stage` the stage
/// it counts toward; says whether the file's text changed.
pub fn count(
    consultation: &mut Consultation,
    event: &Event,
    stage: Option<&str>,
    adapter: Option<&Descriptor>,
) -> bool {
    if !adapter.is_some_and(Descriptor::consults) {
        return false;
    }
    match event.kind.as_str() {
        "tool.start" => {
            let Some(id) = event.call() else {
                return false;
            };
            let (reads, edits) = (classes(event, "reads"), classes(event, "edits"));
            let loft = event.loft();
            let call = Call {
                host: &event.host,
                at: event.at,
                who: &event.who,
                id,
                reads: &reads,
                edits: &edits,
                loft_read: reads_spec(loft.iter().map(String::as_str)),
            };
            consultation.start(&call, stage)
        }
        "tool.end" => {
            if let Some(id) = event.call() {
                consultation.end(&event.who, id);
            }
            false
        }
        "request" => consultation.request(&event.who, event.at, event.tokens(), stage),
        _ => false,
    }
}
