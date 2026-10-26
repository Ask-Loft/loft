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
    all.add(Counts::read(&json!({"input":0,"output":2})).unwrap());
    let first = loft_common::yaml::write(&loft_common::yaml::Out::Mapping(all.fields()));
    assert!(first.contains("input: 0"));
    assert!(!first.contains("cacheRead"));
    assert!(!first.contains("partial"));
    all.add(Counts::unreported());
    all.add(Counts::read(&json!({"output":3})).unwrap());
    let summed = loft_common::yaml::write(&loft_common::yaml::Out::Mapping(all.fields()));
    assert!(summed.contains("output: 5"));
    assert!(summed.contains("partial:\n  - input\n  - output"));
    assert!(!summed.contains("cacheRead"));
    assert!(Counts::read(&json!({"input":-1})).is_err());
    assert!(Counts::read(&json!({"input":"0"})).is_err());
}
