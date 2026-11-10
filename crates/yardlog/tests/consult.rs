//! What a session's agents consulted, as their calls and requests add to it: the six counts by
//! stage and agent, the order of the first consultation of the specification and the first edit
//! of code, the estimated tokens each class brought in, the file's text, and the sums of files.

use serde_json::{Value, json};
use yardlog::Time;
use yardlog::consult::{Call, Class, Consultation, Sums, reads_spec};
use yardlog::totals::Who;
use yardlog::usage::Counts;

const MAIN: Who = Who::Main;

fn at(second: u32) -> Time {
    format!("2026-01-01T10:{:02}:{:02}Z", second / 60, second % 60)
        .parse()
        .unwrap()
}

/// A call of `who` started at `second`.
fn call<'a>(
    who: &'a Who,
    id: &'a str,
    second: u32,
    reads: &'a [Class],
    edits: &'a [Class],
) -> Call<'a> {
    Call {
        at: at(second),
        who,
        id,
        reads,
        edits,
        loft_read: false,
    }
}

fn explore() -> Who {
    Who::Sub {
        id: "a1".to_owned(),
        kind: "Explore".to_owned(),
    }
}

fn tokens(input: u64, read: u64, write: Option<u64>, output: u64) -> Counts {
    let mut usage = json!({"input": input, "cacheRead": read, "output": output});
    if let Some(write) = write {
        usage["cacheWrite"] = json!(write);
    }
    Counts::read(&usage)
}

#[test]
fn a_call_adds_its_classes_to_the_six_counts_in_all_by_stage_and_by_agent() {
    let mut consultation = Consultation::new("s1", "claude-code");
    let sub = explore();
    assert!(consultation.start(&call(&MAIN, "c1", 1, &[Class::Spec], &[]), Some("shape")));
    assert!(consultation.start(&call(&sub, "c2", 2, &[Class::Code], &[]), Some("shape")));
    assert!(consultation.start(&call(&MAIN, "c3", 3, &[], &[Class::Code]), Some("build")));
    assert_eq!(consultation.counts.spec_reads, 1);
    assert_eq!(consultation.counts.code_reads, 1);
    assert_eq!(consultation.counts.code_edits, 1);
    let shape = &consultation.stages["shape"];
    assert_eq!(shape.all.counts.spec_reads + shape.all.counts.code_reads, 2);
    assert_eq!(shape.agents[&MAIN].counts.spec_reads, 1);
    assert_eq!(shape.agents[&sub].counts.code_reads, 1);
    assert_eq!(consultation.stages["build"].all.counts.code_edits, 1);
    let text = consultation.text();
    assert!(
        text.starts_with("format: yardconsult/1\nsession: s1\nhost: claude-code\n"),
        "{text}"
    );
    assert!(
        text.contains("agent: a1\n") && text.contains("type: Explore\n"),
        "{text}"
    );
}

#[test]
fn a_call_whose_reads_hold_two_classes_adds_one_to_the_reads_of_each() {
    let mut consultation = Consultation::new("s1", "claude-code");
    consultation.start(
        &call(
            &MAIN,
            "c1",
            1,
            &[Class::Spec, Class::Code, Class::Other],
            &[],
        ),
        None,
    );
    let counts = consultation.counts;
    assert_eq!(
        (counts.spec_reads, counts.code_reads, counts.other_reads),
        (1, 1, 1)
    );
}

#[test]
fn a_loft_read_adds_one_loft_read_and_other_loft_subcommands_add_none() {
    assert!(reads_spec(["find", "check"]));
    assert!(!reads_spec(["publish", "fmt", "yard"]));
    let mut consultation = Consultation::new("s1", "claude-code");
    let mut loft = call(&MAIN, "c1", 1, &[], &[]);
    loft.loft_read = reads_spec(["find", "check"]);
    assert!(consultation.start(&loft, None));
    assert_eq!(consultation.counts.loft_reads, 1);
    assert_eq!(consultation.counts.spec_consultations(), 1);
    let mut other = call(&MAIN, "c2", 2, &[], &[]);
    other.loft_read = reads_spec(["publish"]);
    assert!(!consultation.start(&other, None));
}

#[test]
fn a_session_that_counted_no_call_has_no_file_to_write() {
    let mut consultation = Consultation::new("s1", "claude-code");
    assert!(!consultation.start(&call(&MAIN, "c1", 1, &[], &[]), Some("shape")));
    assert!(!consultation.counted());
    consultation.start(&call(&MAIN, "c2", 5, &[Class::Code], &[]), Some("shape"));
    consultation.start(&call(&MAIN, "c3", 9, &[Class::Spec], &[]), None);
    assert!(consultation.counted());
    assert_eq!(consultation.first_started, Some(at(5)));
    assert_eq!(consultation.last_started, Some(at(9)));
    assert_eq!(
        consultation
            .no_stage
            .as_ref()
            .unwrap()
            .all
            .counts
            .spec_reads,
        1
    );
}

#[test]
fn a_stage_counts_the_edits_of_code_made_before_the_specification_was_first_consulted() {
    let mut consultation = Consultation::new("s1", "claude-code");
    consultation.start(&call(&MAIN, "c1", 1, &[], &[Class::Code]), Some("build"));
    consultation.start(&call(&MAIN, "c2", 2, &[], &[Class::Code]), Some("build"));
    consultation.start(&call(&MAIN, "c3", 3, &[Class::Spec], &[]), Some("build"));
    consultation.start(&call(&MAIN, "c4", 4, &[], &[Class::Code]), Some("build"));
    let order = consultation.stages["build"].all.order;
    assert_eq!(order.first_code_edit, Some(at(1)));
    assert_eq!(order.first_spec, Some(at(3)));
    assert_eq!(order.code_edits_before_spec, 2);
    assert_eq!(order.spec_first(), Some(false));
}

#[test]
fn a_stage_whose_specification_was_consulted_first_counts_no_edit_before_it() {
    let mut consultation = Consultation::new("s1", "claude-code");
    let mut loft = call(&MAIN, "c1", 1, &[], &[]);
    loft.loft_read = true;
    consultation.start(&loft, Some("build"));
    consultation.start(&call(&MAIN, "c2", 2, &[], &[Class::Code]), Some("build"));
    let order = consultation.stages["build"].all.order;
    assert_eq!(order.code_edits_before_spec, 0);
    assert_eq!(order.spec_first(), Some(true));
}

#[test]
fn a_stage_with_no_edit_of_code_or_no_consultation_holds_no_time_for_it() {
    let mut consultation = Consultation::new("s1", "claude-code");
    consultation.start(&call(&MAIN, "c1", 1, &[Class::Code], &[]), Some("shape"));
    let order = consultation.stages["shape"].all.order;
    assert_eq!((order.first_spec, order.first_code_edit), (None, None));
    assert_eq!(order.spec_first(), None);
    let text = consultation.text();
    assert!(
        !text.contains("firstSpec") && !text.contains("firstCodeEdit"),
        "{text}"
    );
}

#[test]
fn a_stage_s_order_takes_every_agent_s_calls_and_an_agent_s_only_its_own() {
    let mut consultation = Consultation::new("s1", "claude-code");
    let sub = explore();
    consultation.start(&call(&sub, "c1", 1, &[Class::Spec], &[]), Some("build"));
    consultation.start(&call(&MAIN, "c2", 2, &[], &[Class::Code]), Some("build"));
    let stage = &consultation.stages["build"];
    assert_eq!(stage.all.order.code_edits_before_spec, 0);
    assert_eq!(stage.agents[&MAIN].order.code_edits_before_spec, 1);
    assert_eq!(stage.agents[&sub].order.first_spec, Some(at(1)));
}

#[test]
fn the_file_holds_when_its_last_event_happened_for_a_merge_to_take_the_later_copy() {
    let mut consultation = Consultation::new("s1", "claude-code");
    consultation.start(&call(&MAIN, "c1", 1, &[Class::Spec], &[]), Some("shape"));
    consultation.end(&MAIN, "c1");
    consultation.request(&MAIN, at(2), Some(tokens(10, 100, None, 5)), Some("shape"));
    consultation.start(&call(&MAIN, "c2", 3, &[], &[]), Some("shape"));
    consultation.end(&MAIN, "c2");
    assert!(consultation.request(&MAIN, at(4), Some(tokens(10, 160, None, 5)), Some("shape")));
    assert_eq!(consultation.last_ended, Some(at(4)));
    assert!(
        consultation
            .text()
            .contains("lastEnded: 2026-01-01T10:00:04Z\n")
    );
}

#[test]
fn a_request_s_prompt_growth_less_the_last_output_is_shared_among_the_calls_ended_since() {
    let mut consultation = Consultation::new("s1", "claude-code");
    consultation.request(
        &MAIN,
        at(1),
        Some(tokens(10, 1000, Some(0), 100)),
        Some("build"),
    );
    let mut loft = call(&MAIN, "c3", 4, &[], &[]);
    loft.loft_read = true;
    for started in [
        call(&MAIN, "c1", 2, &[Class::Spec, Class::Code], &[]),
        call(&MAIN, "c2", 3, &[], &[]),
        loft,
    ] {
        consultation.start(&started, Some("build"));
        consultation.end(&MAIN, started.id);
    }
    // 1700 - (1010 + 100) = 590 grown: 197, 197, 196 to the three calls.
    assert!(consultation.request(
        &MAIN,
        at(5),
        Some(tokens(10, 1600, Some(90), 7)),
        Some("build")
    ));
    let estimate = consultation.stages["build"].agents[&MAIN].estimate;
    assert_eq!(estimate.code, 98);
    assert_eq!(estimate.spec, 99 + 196);
    assert_eq!(estimate.unclassified, 197);
    assert_eq!(consultation.estimate, estimate);
}

#[test]
fn an_agent_s_first_request_a_growth_of_none_and_a_request_with_no_call_ended_attribute_nothing() {
    let mut consultation = Consultation::new("s1", "claude-code");
    consultation.start(&call(&MAIN, "c1", 1, &[Class::Spec], &[]), None);
    consultation.end(&MAIN, "c1");
    assert!(!consultation.request(&MAIN, at(2), Some(tokens(10, 500, None, 50)), None));
    assert!(!consultation.request(&MAIN, at(3), Some(tokens(10, 1000, None, 50)), None));
    consultation.start(&call(&MAIN, "c2", 4, &[Class::Code], &[]), None);
    consultation.end(&MAIN, "c2");
    assert!(!consultation.request(&MAIN, at(5), Some(tokens(10, 900, None, 50)), None));
    assert_eq!(consultation.estimate.spec + consultation.estimate.code, 0);
    assert!(!consultation.partial);
}

#[test]
fn a_request_that_lacks_a_count_or_follows_one_that_does_attributes_nothing_and_marks_partial() {
    let mut consultation = Consultation::new("s1", "claude-code");
    consultation.request(&MAIN, at(1), Some(tokens(10, 100, None, 5)), None);
    consultation.start(&call(&MAIN, "c1", 2, &[Class::Spec], &[]), None);
    consultation.end(&MAIN, "c1");
    let lacking = Some(Counts::read(&json!({"input": 10, "output": 5})));
    assert!(consultation.request(&MAIN, at(3), lacking, None));
    assert!(consultation.partial);
    consultation.start(&call(&MAIN, "c2", 4, &[Class::Spec], &[]), None);
    consultation.end(&MAIN, "c2");
    consultation.request(&MAIN, at(5), Some(tokens(10, 900, None, 5)), None);
    assert_eq!(consultation.estimate.spec, 0);
    assert!(consultation.text().contains("partial: true\n"));
}

#[test]
fn a_cache_write_a_request_does_not_report_counts_as_none() {
    let mut consultation = Consultation::new("s1", "claude-code");
    consultation.request(&MAIN, at(1), Some(tokens(10, 100, None, 5)), None);
    consultation.start(&call(&MAIN, "c1", 2, &[Class::Code], &[]), None);
    consultation.end(&MAIN, "c1");
    consultation.request(&MAIN, at(3), Some(tokens(10, 145, None, 5)), None);
    assert_eq!(consultation.estimate.code, 40);
    assert!(!consultation.partial);
}

#[test]
fn a_subagent_s_calls_go_to_its_own_requests_not_the_main_agent_s() {
    let mut consultation = Consultation::new("s1", "claude-code");
    let sub = explore();
    consultation.request(&MAIN, at(1), Some(tokens(10, 100, None, 5)), Some("shape"));
    consultation.request(&sub, at(1), Some(tokens(10, 100, None, 5)), Some("shape"));
    consultation.start(&call(&sub, "c1", 2, &[Class::Spec], &[]), Some("shape"));
    consultation.end(&sub, "c1");
    consultation.request(&MAIN, at(3), Some(tokens(10, 200, None, 5)), Some("shape"));
    assert_eq!(consultation.estimate.spec, 0);
    consultation.request(&sub, at(4), Some(tokens(10, 200, None, 5)), Some("shape"));
    assert_eq!(consultation.stages["shape"].agents[&sub].estimate.spec, 95);
}

/// The activity of session `session` under a descriptor that declares `consults`, or not.
fn activity(session: &str, consults: bool) -> Value {
    let mut capabilities = json!({
        "events": [], "usage": [], "nativeQuestions": false, "nativePlans": false,
        "taskMirroring": false,
    });
    if consults {
        capabilities["consults"] = json!(true);
    }
    json!({
        "session": session, "host": "claude-code",
        "adapter": {"format": "yard-adapter/1", "version": "1.0.0", "capabilities": capabilities},
    })
}

#[test]
fn the_sums_take_the_earliest_times_and_each_session_s_edits_before_the_specification() {
    let mut first = Consultation::new("s1", "claude-code");
    first.start(&call(&MAIN, "c1", 5, &[], &[Class::Code]), Some("build"));
    first.start(&call(&MAIN, "c2", 6, &[Class::Spec], &[]), Some("build"));
    let mut second = Consultation::new("s2", "claude-code");
    second.start(&call(&MAIN, "c1", 2, &[Class::Spec], &[]), Some("build"));
    second.start(&call(&MAIN, "c2", 3, &[], &[Class::Code]), Some("build"));
    second.partial = true;
    let mut summed = Sums::default();
    summed.add(&first);
    summed.add(&second);
    summed.describe(&activity("s1", true));
    summed.describe(&activity("s3", false));
    let build = summed.stages["build"];
    assert_eq!(build.order.first_spec, Some(at(2)));
    assert_eq!(build.order.first_code_edit, Some(at(3)));
    assert_eq!(build.order.code_edits_before_spec, 1);
    assert_eq!(build.order.spec_first(), Some(false));
    assert_eq!(build.counts.spec_reads, 2);
    assert_eq!(summed.counts.code_edits, 2);
    assert!(summed.partial);
    assert_eq!(summed.sessions.len(), 2);
    assert_eq!(summed.agents.keys().collect::<Vec<_>>(), ["main"]);
    assert_eq!(
        summed.unmeasured,
        [("s3".to_owned(), "claude-code".to_owned())]
    );
    assert!(summed.measured());
}

#[test]
fn the_sums_by_agent_type_take_each_subagent_of_a_type_together() {
    let mut consultation = Consultation::new("s1", "claude-code");
    let two = Who::Sub {
        id: "a2".into(),
        kind: "Explore".into(),
    };
    consultation.start(
        &call(&explore(), "c1", 1, &[Class::Spec], &[]),
        Some("shape"),
    );
    consultation.start(&call(&two, "c2", 2, &[Class::Spec], &[]), None);
    let mut summed = Sums::default();
    summed.add(&consultation);
    assert_eq!(summed.agents.keys().collect::<Vec<_>>(), ["Explore"]);
    assert_eq!(summed.agents["Explore"].counts.spec_reads, 2);
    assert!(!Sums::default().measured());
}

#[test]
fn the_sums_leave_out_a_file_of_another_format_and_are_partial() {
    let newer = json!({"format": "yardconsult/2", "session": "s1", "counts": {"specReads": 4}});
    let summed = Sums::of(&[newer], &[]);
    assert!(summed.partial);
    assert!(!summed.measured());
    assert_eq!(summed.counts.spec_reads, 0);
}

#[test]
fn a_call_still_open_across_a_whole_request_is_let_go_and_one_open_across_none_is_kept() {
    let mut consultation = Consultation::new("s1", "claude-code");
    consultation.start(&call(&MAIN, "refused", 1, &[Class::Spec], &[]), None);
    consultation.request(&MAIN, at(2), Some(tokens(10, 100, None, 5)), None);
    consultation.start(&call(&MAIN, "late", 3, &[Class::Code], &[]), None);
    consultation.request(&MAIN, at(4), Some(tokens(10, 100, None, 5)), None);
    // The refused call never ended, and was open across a whole request; the late one ends after
    // the request it ran beside, as a host that hears usage apart from its calls may tell it.
    consultation.end(&MAIN, "refused");
    consultation.end(&MAIN, "late");
    consultation.request(&MAIN, at(5), Some(tokens(10, 300, None, 5)), None);
    assert_eq!(consultation.estimate.spec, 0);
    assert_eq!(consultation.estimate.code, 195);
}
