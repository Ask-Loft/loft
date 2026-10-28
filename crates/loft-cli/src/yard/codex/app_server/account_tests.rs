//! Source classification is whitelisted independently of credentials, counts and native activity.

use serde_json::{Value, json};
use software_plant::codex::{CodexAppServer, Scenario};

use super::{Connection, Notice};

fn usage(input: u64) -> Value {
    json!({"method":"thread/tokenUsage/updated","params":{"threadId":"thread","turnId":"turn",
        "tokenUsage":{"last":{"inputTokens":input,"cachedInputTokens":30,"cacheWriteInputTokens":0,"outputTokens":5},
        "total":{"inputTokens":input,"cachedInputTokens":30,"cacheWriteInputTokens":0,"outputTokens":5}}}})
}

fn observations(scenario: Scenario) -> (Vec<Value>, Vec<Value>) {
    let peer = CodexAppServer::start(scenario);
    let mut connection = Connection::connect(peer.path(), "thread").unwrap();
    connection
        .request("thread/loaded/list", &json!({}))
        .unwrap();
    let observed = connection
        .poll()
        .unwrap()
        .into_iter()
        .filter_map(|notice| match notice {
            Notice::Usage { usage, .. } => Some(usage),
            Notice::End { .. } => None,
        })
        .collect();
    (observed, peer.requests())
}

#[test]
fn account_lookup_retains_only_verified_source_classification_without_requesting_refresh() {
    for (account, expected) in [
        (
            Some(json!({"type":"apiKey","secret":"PRIVATE KEY"})),
            "apiKey",
        ),
        (
            Some(
                json!({"type":"chatgpt","email":"PRIVATE EMAIL","planType":"pro","accessToken":"PRIVATE TOKEN"}),
            ),
            "chatgpt",
        ),
        (
            Some(json!({"type":"future","email":"PRIVATE EMAIL"})),
            "unknown",
        ),
        (None, "unknown"),
    ] {
        let (observed, requests) = observations(Scenario {
            loaded: vec!["thread".into()],
            account,
            on_subscribe: vec![usage(100)],
            ..Scenario::default()
        });
        assert_eq!(observed.len(), 1);
        assert_eq!(observed[0]["account"], expected);
        assert_eq!(observed[0]["last"]["cacheWriteInputTokens"], 0);
        assert!(!observed[0].to_string().contains("PRIVATE"));
        let lookup = requests
            .iter()
            .find(|r| r["method"] == "account/read")
            .unwrap();
        assert_eq!(lookup["params"], json!({"refreshToken":false}));
        assert!(!requests.iter().any(|r| {
            r["method"]
                .as_str()
                .is_some_and(|m| m.starts_with("account/login") || m == "account/logout")
        }));
    }
}

#[test]
fn refused_account_inspection_leaves_other_counters_and_the_subscription_available() {
    let (observed, requests) = observations(Scenario {
        loaded: vec!["thread".into()],
        refuse_methods: vec!["account/read".into()],
        on_subscribe: vec![usage(100)],
        ..Scenario::default()
    });
    assert_eq!(observed[0]["account"], "unknown");
    assert_eq!(observed[0]["last"]["inputTokens"], 100);
    assert_eq!(observed[0]["last"]["cachedInputTokens"], 30);
    assert_eq!(observed[0]["last"]["outputTokens"], 5);
    assert!(requests.iter().any(|r| r["method"] == "thread/resume"));
}

#[test]
fn account_updates_apply_to_subsequent_usage_without_retaining_account_details() {
    let account = |mode| json!({"method":"account/updated","params":{"authMode":mode,"email":"PRIVATE EMAIL","accessToken":"PRIVATE TOKEN"}});
    let (observed, _) = observations(Scenario {
        loaded: vec!["thread".into()],
        account: Some(json!({"type":"chatgpt"})),
        on_subscribe: vec![
            account("apikey"),
            usage(100),
            account("chatgptAuthTokens"),
            usage(200),
            account("future"),
            usage(300),
        ],
        ..Scenario::default()
    });
    assert_eq!(
        observed
            .iter()
            .map(|v| v["account"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["apiKey", "chatgpt", "unknown"]
    );
    assert!(
        !serde_json::to_string(&observed)
            .unwrap()
            .contains("PRIVATE")
    );
}
