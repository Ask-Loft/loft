//! How much of an answer Loft reads: 64 MiB of a remote's, and 1 MiB of an answer to a
//! sign-in's request for metadata, a registration or tokens. An answer as long as its limit is
//! read whole, and a longer one is refused. Each runs under the remote and git simulators, with
//! a server that pads every answer to one length.

use serde_json::{Value, json};
use software_plant::http::{Answer, Request, Responder, Response, Server};
use software_plant::remote::{RemoteScenario, TOKEN, document_address};

use crate::common::connected::{Connected, connected};
use crate::common::signed_in::{granted, keep};
use crate::common::tree;

/// Answers every request with one status and one JSON body, padded with spaces to a length.
struct Padded(u16, Value, usize);

impl Responder for Padded {
    fn answer(&mut self, _: &Request) -> Answer {
        let mut response = Response::json(self.0, &self.1);
        response.body.resize(self.2, b' ');
        response.into()
    }
}

/// The message of the refused publish, after asserting that it wrote nothing.
fn refused(run: &Connected, env: &[(&str, &str)]) -> String {
    let before = tree(&run.scratch.0);
    let output = run.publish_with(env);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert_eq!(tree(&run.scratch.0), before, "nothing is written");
    Connected::message(&output)
}

#[test]
fn an_answer_of_a_remote_is_read_up_to_64_mib_and_refused_past_it() {
    const LIMIT: usize = 64 << 20;
    let run = connected(1, 1, RemoteScenario::accepting);
    run.change();
    for (length, said, unsaid) in [
        (LIMIT, "conflict", "longer than"),
        (LIMIT + 1, "longer than 67108864 bytes", "conflict"),
    ] {
        let server = Server::start(Padded(409, json!({ "code": "conflict" }), length));
        let address = document_address(&server.url(), &run.history.key);
        let message = refused(
            &run,
            &[
                ("LOFT_REMOTE_ENDPOINT", &address),
                ("LOFT_REMOTE_TOKEN", TOKEN),
            ],
        );
        assert!(message.contains(said), "{length}: {message}");
        assert!(!message.contains(unsaid), "{length}: {message}");
        assert!(message.contains(&address), "{length}: {message}");
    }
}

#[test]
fn an_answer_to_a_sign_in_s_request_is_read_up_to_1_mib_and_refused_past_it() {
    const LIMIT: usize = 1 << 20;
    let run = connected(1, 1, RemoteScenario::accepting);
    run.change();
    for (length, said, unsaid) in [
        (LIMIT, "invalid_grant", "longer than"),
        (LIMIT + 1, "longer than 1048576 bytes", "invalid_grant"),
    ] {
        let server = Server::start(Padded(400, json!({ "error": "invalid_grant" }), length));
        // The kept sign-in is about to expire, so it is refreshed first, at the server above.
        let mut kept = granted(&run.remote, 60);
        let endpoint = format!("{}/token", server.url());
        kept["tokenEndpoint"] = json!(endpoint);
        keep(&run.scratch, &json!({ run.remote.address(): kept }));
        let message = refused(&run, &[]);
        assert!(message.contains(said), "{length}: {message}");
        assert!(!message.contains(unsaid), "{length}: {message}");
        // The error names the address that gave the answer where it is too long, and the
        // remote's own where the sign-in lapsed.
        let named = if length > LIMIT {
            endpoint
        } else {
            run.remote.address()
        };
        assert!(message.contains(&named), "{length}: {message}");
        assert!(run.remote.requests().is_empty(), "{length}");
    }
}
