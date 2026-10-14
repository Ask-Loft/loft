//! `loft telemetry on|off|status` and `loft update-check on|off|status`: record the user's
//! choice on the machine, and report the state the switches give. Neither reads a tree, sends
//! an event or asks GitHub anything.

use loftspec::publish::PublishedAt;
use serde_json::{Map, Value, json};

use crate::endpoints;
use crate::finish::Finished;
use crate::json;
use crate::out::outln;
use crate::settings;
use crate::stop::Stop;
use crate::switches::{Switch, TELEMETRY, UPDATE_CHECK};

/// What the command does.
#[derive(Clone, Copy)]
pub enum Action {
    On,
    Off,
    Status,
}

/// A report's further fields, and its further lines of text.
type Details = (Map<String, Value>, Vec<String>);

/// A switched concern as its command shows it.
struct Shown {
    switch: &'static Switch,
    schema: &'static str,
    title: &'static str,
    /// Settings forgotten when the concern is switched off.
    forgotten: &'static [&'static str],
    /// The report's further fields and text lines, from the concern's settings.
    details: fn(&Value) -> Details,
}

const TELEMETRY_SHOWN: Shown = Shown {
    switch: &TELEMETRY,
    schema: "loft-telemetry/1",
    title: "telemetry",
    forgotten: &["installation"],
    details: telemetry_details,
};

const UPDATE_CHECK_SHOWN: Shown = Shown {
    switch: &UPDATE_CHECK,
    schema: "loft-update-check/1",
    title: "update check",
    forgotten: &[],
    details: update_check_details,
};

/// `loft telemetry <action>`.
pub fn telemetry(action: Action, as_json: bool) -> Finished {
    run(&TELEMETRY_SHOWN, action, as_json)
}

/// `loft update-check <action>`.
pub fn update_check(action: Action, as_json: bool) -> Finished {
    run(&UPDATE_CHECK_SHOWN, action, as_json)
}

fn run(shown: &Shown, action: Action, as_json: bool) -> Finished {
    let choice = match action {
        Action::On => Some("on"),
        Action::Off => Some("off"),
        Action::Status => None,
    };
    if let Some(choice) = choice
        && let Err(error) = record(shown, choice)
    {
        return Stop::one("settings", error.to_string()).exit(shown.schema, as_json);
    }
    let state = shown.switch.state();
    let outcome = if state.on { "on" } else { "off" };
    let (mut fields, lines) = (shown.details)(&settings::read(shown.switch.file));
    if as_json {
        fields.insert("decidedBy".to_owned(), json!(state.decided_by));
        let document = json::result(shown.schema, outcome, Value::Object(fields), Vec::new());
        json::print(&document);
    } else {
        let by = if state.decided_by == "default" {
            "by default".to_owned()
        } else {
            format!("by `{}`", state.decided_by)
        };
        outln!("{}: {outcome}, {by}", shown.title);
        for line in lines {
            outln!("{line}");
        }
    }
    Finished::success(outcome)
}

/// Records `choice` in the concern's settings, forgetting what switching it off forgets.
fn record(shown: &Shown, choice: &str) -> Result<(), settings::SettingsError> {
    let mut kept = settings::read(shown.switch.file);
    kept["choice"] = json!(choice);
    if choice == "off"
        && let Some(object) = kept.as_object_mut()
    {
        for name in shown.forgotten {
            object.remove(*name);
        }
    }
    settings::write(shown.switch.file, &kept)
}

fn telemetry_details(kept: &Value) -> Details {
    let collector = endpoints::collector();
    let shown = kept["noticeShown"].as_bool().unwrap_or(false);
    let mut fields = Map::new();
    fields.insert("collector".to_owned(), json!(collector));
    fields.insert("noticeShown".to_owned(), json!(shown));
    let notice = if shown { "printed" } else { "not printed yet" };
    let lines = vec![
        format!("collector: {collector}"),
        format!("notice: {notice}"),
    ];
    (fields, lines)
}

fn update_check_details(kept: &Value) -> Details {
    let mut fields = Map::new();
    let received = kept["receivedAt"]
        .as_u64()
        .and_then(|at| PublishedAt::from_unix_seconds(at).ok());
    let line = match kept["latest"]["version"].as_str().zip(received) {
        Some((version, received)) => {
            let received = received.to_string();
            fields.insert(
                "latest".to_owned(),
                json!({ "version": version, "receivedAt": received }),
            );
            format!("latest release: {version}, received {received}")
        }
        None => "no answer from GitHub is held".to_owned(),
    };
    (fields, vec![line])
}
