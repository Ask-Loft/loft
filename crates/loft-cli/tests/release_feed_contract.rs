//! The contract suite of the release feed simulator: each scenario asks the simulator and
//! GitHub's REST API the same question and requires answers of the same shape. The real half is
//! ignored by default, since it reaches GitHub without a token, whose limit is 60 requests an
//! hour for an address; run it with
//! `cargo nextest run -p loft-cli --run-ignored only -E 'test(/release_feed_contract/)'`.

use std::time::Duration;

use serde_json::Value;
use software_plant::releases::{LATEST, Release, ReleaseFeed};
use ureq::Agent;

/// GitHub's REST API.
const GITHUB: &str = "https://api.github.com";

fn agent() -> Agent {
    Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(10)))
        .build()
        .into()
}

/// The status of `GET <api><path>`, with its JSON body.
fn get(api: &str, path: &str) -> (u16, Value) {
    let mut response = agent()
        .get(format!("{api}{path}"))
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", concat!("loft/", env!("CARGO_PKG_VERSION")))
        .call()
        .expect("the feed answers");
    let status = response.status().as_u16();
    let body = response.body_mut().read_to_vec().unwrap_or_default();
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

/// What the suite compares of the latest release: the status, a tag of the form `vX.Y.Z`, a
/// page under the repository's releases, and both flags false.
fn shape(answer: &(u16, Value)) -> (u16, bool, bool, Value, Value) {
    let (status, body) = answer;
    let tag = body["tag_name"].as_str().unwrap_or_default();
    let numbers: Vec<&str> = tag
        .strip_prefix('v')
        .unwrap_or_default()
        .split('.')
        .collect();
    let versioned = numbers.len() == 3
        && numbers
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()));
    let page = body["html_url"]
        .as_str()
        .is_some_and(|page| page == format!("https://github.com/Ask-Loft/loft/releases/tag/{tag}"));
    (
        *status,
        versioned,
        page,
        body["draft"].clone(),
        body["prerelease"].clone(),
    )
}

/// The feed GitHub's answers are compared with: one published release.
fn simulated() -> ReleaseFeed {
    ReleaseFeed::listing(vec![Release::published("v2.0.3")])
}

#[test]
fn the_simulator_answers_the_latest_release_in_github_s_shape() {
    let feed = simulated();
    let answer = get(&feed.url(), LATEST);
    assert_eq!(
        shape(&answer),
        (200, true, true, Value::Bool(false), Value::Bool(false))
    );
}

#[test]
fn the_simulator_answers_another_repository_with_404() {
    let feed = simulated();
    assert_eq!(
        get(&feed.url(), "/repos/Ask-Loft/no-such-repo/releases/latest").0,
        404
    );
}

#[test]
fn the_simulator_skips_a_newer_draft_and_a_newer_prerelease_as_github_does() {
    let feed = ReleaseFeed::listing(vec![
        Release::draft("v2.2.0"),
        Release::prerelease("v2.1.0"),
        Release::published("v2.0.3"),
    ]);
    assert_eq!(get(&feed.url(), LATEST).1["tag_name"], "v2.0.3");
}

#[test]
fn a_misreporting_simulator_answers_its_release_whatever_its_flags() {
    let feed = ReleaseFeed::answering(Release::prerelease("v9.0.0"));
    let (_, body) = get(&feed.url(), LATEST);
    assert_eq!(body["prerelease"], Value::Bool(true));
}

#[test]
fn a_simulator_can_list_a_release_whose_tag_is_not_a_version() {
    let feed = ReleaseFeed::listing(vec![Release::published("nightly")]);
    let answer = get(&feed.url(), LATEST);
    assert_eq!(answer.1["tag_name"], "nightly");
    assert!(!shape(&answer).1);
}

#[test]
fn a_rate_limited_simulator_answers_as_github_does_when_the_limit_is_spent() {
    let feed = ReleaseFeed::rate_limited();
    let mut response = agent()
        .get(format!("{}{LATEST}", feed.url()))
        .call()
        .unwrap();
    assert_eq!(response.status().as_u16(), 403);
    assert_eq!(
        response.headers()["x-ratelimit-remaining"]
            .to_str()
            .unwrap(),
        "0"
    );
    let body: Value = serde_json::from_slice(&response.body_mut().read_to_vec().unwrap()).unwrap();
    assert!(
        body["message"].as_str().unwrap().contains("rate limit"),
        "{body}"
    );
}

#[test]
fn a_failing_simulator_answers_with_its_status() {
    let feed = ReleaseFeed::failing(500);
    assert_eq!(get(&feed.url(), LATEST).0, 500);
}

#[test]
fn a_silent_simulator_never_answers_and_a_refusing_one_refuses() {
    let feed = ReleaseFeed::silent();
    let quiet: Agent = Agent::config_builder()
        .timeout_global(Some(Duration::from_millis(300)))
        .build()
        .into();
    assert!(quiet.get(format!("{}{LATEST}", feed.url())).call().is_err());
    let refused = agent()
        .get(format!("{}{LATEST}", ReleaseFeed::refusing()))
        .call();
    assert!(refused.is_err(), "{refused:?}");
}

#[test]
fn the_simulator_counts_the_requests_it_receives() {
    let feed = simulated();
    get(&feed.url(), LATEST);
    get(&feed.url(), LATEST);
    assert_eq!(feed.requests().len(), 2);
}

#[test]
#[ignore = "reaches GitHub's REST API"]
fn github_answers_each_scenario_in_the_shape_the_simulator_does() {
    let feed = simulated();
    for path in [LATEST, "/repos/Ask-Loft/no-such-repo/releases/latest"] {
        let real = shape(&get(GITHUB, path));
        let simulated = shape(&get(&feed.url(), path));
        assert_eq!(real, simulated, "{path}");
    }
}
