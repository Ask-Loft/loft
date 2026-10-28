//! The adapter contract and counter semantics are pure model behavior.

use serde_json::json;
use yardlog::adapter::Descriptor;
use yardlog::usage::Counts;

#[test]
fn an_empty_capability_declaration_is_known_unavailable_and_normalizes_to_empty_lists() {
    let descriptor =
        Descriptor::read(&json!({"format":"yard-adapter/1", "version":"1", "capabilities":{}}))
            .unwrap();
    let normalized = descriptor.json();
    assert_eq!(normalized["capabilities"]["events"], json!([]));
    assert_eq!(normalized["capabilities"]["usage"], json!([]));
    assert_eq!(normalized["capabilities"]["nativeQuestions"], false);
    assert_eq!(Descriptor::read(&normalized).unwrap(), descriptor);
    let yaml = loft_common::yaml::write(&descriptor.yaml());
    assert!(yaml.contains("nativeQuestions: false"));
    assert!(!yaml.contains("events:"));
}

#[test]
fn unreported_counters_remain_absent_but_measured_zero_survives_aggregation() {
    let mut all = Counts::default();
    all.add(Counts::read(&json!({"input":0,"output":2})));
    let first = loft_common::yaml::write(&loft_common::yaml::Out::Mapping(all.fields()));
    assert!(first.contains("input: 0"));
    assert!(!first.contains("cacheRead"));
    assert!(!first.contains("partial"));
    all.add(Counts::unreported());
    all.add(Counts::read(&json!({"output":3})));
    let summed = loft_common::yaml::write(&loft_common::yaml::Out::Mapping(all.fields()));
    assert!(summed.contains("output: 5"));
    assert!(summed.contains("partial:\n  - input\n  - output"));
    assert!(!summed.contains("cacheRead"));
}

#[test]
fn a_counter_that_is_not_a_whole_number_is_unreported_beside_the_counters_measured() {
    for unusable in [json!(null), json!(-1), json!(1.5), json!("0"), json!([1])] {
        let mut all = Counts::read(&json!({"input":3}));
        all.add(Counts::read(&json!({"input":unusable,"output":2})));
        let yaml = loft_common::yaml::write(&loft_common::yaml::Out::Mapping(all.fields()));
        assert_eq!(
            yaml, "input: 3\noutput: 2\npartial:\n  - input\n  - output\n",
            "{unusable}"
        );
    }
    for unusable in [json!(null), json!(7), json!("tokens"), json!([1])] {
        assert_eq!(Counts::read(&unusable), Counts::unreported(), "{unusable}");
    }
}
