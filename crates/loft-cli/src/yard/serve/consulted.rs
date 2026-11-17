//! What a session's agents consulted, as the page server counts it from its events (the Feature
//! "Count what the agents consulted"): a tool call's start adds its classes and its `loft` reads,
//! its end readies its result for the agent's next request, and a model request attributes the
//! growth of its prompt to the calls that ended before it. The estimate's state is the session's,
//! whichever Yard Log keeps its events, and a subagent's is let go as its run ends. Only a session
//! whose adapter's descriptor declares `consults` is counted.

use yardlog::adapter::Descriptor;
use yardlog::consult::{Call, Class, Consultation, Live, reads_spec};
use yardlog::totals::Who;

use crate::yard::event::{Event, strings};

/// The classes `event` names in its field `name`, each it knows.
fn classes(event: &Event, name: &str) -> Vec<Class> {
    strings(&event.posted[name])
        .iter()
        .filter_map(|class| Class::named(class))
        .collect()
}

/// Counts `event` into `consultation`, and the session's estimate state `live`, under the
/// session's descriptor `adapter`, `stage` the stage it counts toward; says whether the file's
/// text changed.
pub fn count(
    consultation: &mut Consultation,
    live: &mut Live,
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
                at: event.at,
                who: &event.who,
                id,
                reads: &reads,
                edits: &edits,
                loft_read: reads_spec(loft.iter().map(String::as_str)),
            };
            live.started(&call);
            consultation.start(&call, stage)
        }
        "tool.end" => {
            if let Some(id) = event.call() {
                live.ended(&event.who, id);
            }
            false
        }
        "request" => {
            let attributed = live.requested(&event.who, event.tokens());
            consultation.request(&event.who, event.at, attributed, stage)
        }
        "subagent.end" if matches!(event.who, Who::Sub { .. }) => {
            live.forget(&event.who);
            false
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use yardlog::Time;
    use yardlog::consult::{Call, Class, Consultation, Live};
    use yardlog::totals::Who;
    use yardlog::usage::Counts;

    use crate::yard::session_files::as_read;

    fn at(second: u32) -> Time {
        format!("2026-01-01T10:00:{second:02}Z").parse().unwrap()
    }

    #[test]
    fn a_consultation_file_reads_back_as_it_was_written() {
        let (mut consultation, mut live) = (Consultation::new("s1", "codex"), Live::default());
        let sub = Who::Sub {
            id: "a1".to_owned(),
            kind: "Explore".to_owned(),
        };
        let usage = |read| {
            Some(Counts::read(
                &serde_json::json!({"input": 10, "cacheRead": read, "output": 5}),
            ))
        };
        let calls = [
            (
                &Who::Main,
                "c1",
                Some("build"),
                [Class::Spec].as_slice(),
                [].as_slice(),
            ),
            (&Who::Main, "c2", Some("build"), &[], &[Class::Code]),
            (&sub, "c3", None, &[Class::Code, Class::Other], &[]),
        ];
        let main = live.requested(&Who::Main, usage(100));
        consultation.request(&Who::Main, at(1), main, Some("build"));
        for (second, (who, id, stage, reads, edits)) in (2..).zip(calls) {
            let call = Call {
                at: at(second),
                who,
                id,
                reads,
                edits,
                loft_read: false,
            };
            live.started(&call);
            consultation.start(&call, stage);
            live.ended(who, id);
        }
        let main = live.requested(&Who::Main, usage(500));
        consultation.request(&Who::Main, at(8), main, Some("build"));
        let lacking = live.requested(&sub, None);
        consultation.request(&sub, at(9), lacking, None);
        let text = consultation.text();
        let read = Consultation::read(&as_read(&text)).expect("a file of its own format");
        assert_eq!(read.text(), text);
        assert!(
            text.contains("partial: true\n") && text.contains("noStage:\n"),
            "{text}"
        );
    }
}
