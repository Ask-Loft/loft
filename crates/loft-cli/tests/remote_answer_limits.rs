//! How much of an answer Loft reads: 64 MiB of a remote's, and 1 MiB of an answer to a
//! sign-in's request for metadata, a registration or tokens. An answer as long as its limit is
//! read whole, and a longer one is refused. Each runs under the remote and git simulators, with
//! a server that pads its answer to one length.

use serde_json::{Value, json};
use software_plant::BrowserScenario;
use software_plant::http::{Answer, Request, Responder, Response, Server};
use software_plant::remote::{RemoteScenario, TOKEN, document_address};

use crate::common::connected::{Connected, connected};
use crate::common::remote::Always;
use crate::common::signed_in::{name_remote, run};
use crate::common::{Scratch, stderr, tree};

/// A response of `status` whose JSON `body` is padded with spaces to `length` bytes.
fn padded(status: u16, body: &Value, length: usize) -> Response {
    let mut response = Response::json(status, body);
    response.body.resize(length, b' ');
    response
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
        let refusal = padded(409, &json!({ "code": "conflict" }), length);
        let server = Server::start(Always(refusal));
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
fn a_token_endpoint_s_answer_is_read_up_to_1_mib_and_refused_past_it() {
    const LIMIT: usize = 1 << 20;
    let run = connected(1, 1, RemoteScenario::accepting);
    run.change();
    for (length, said, unsaid) in [
        (LIMIT, "invalid_grant", "longer than"),
        (LIMIT + 1, "longer than 1048576 bytes", "invalid_grant"),
    ] {
        let refusal = padded(400, &json!({ "error": "invalid_grant" }), length);
        let server = Server::start(Always(refusal));
        let endpoint = format!("{}/token", server.url());
        run.refreshing_at(&endpoint);
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

/// A remote that challenges a request for its head, naming its metadata on its own origin, and
/// answers every other request with one response.
struct Challenging(Response);

impl Responder for Challenging {
    fn answer(&mut self, request: &Request) -> Answer {
        if !request.path.ends_with("/head") {
            return self.0.clone().into();
        }
        let host = request.header("host").unwrap_or_default();
        let named = format!("Bearer resource_metadata=\"http://{host}/metadata\"");
        let mut response = Response::empty(401);
        response.headers.push(("WWW-Authenticate".into(), named));
        response.into()
    }
}

#[test]
fn a_sign_in_s_metadata_is_read_up_to_1_mib_and_refused_past_it() {
    const LIMIT: usize = 1 << 20;
    for (length, longer) in [(LIMIT, false), (LIMIT + 1, true)] {
        let server = Server::start(Challenging(padded(200, &json!({}), length)));
        let scratch = Scratch::with_reading_list();
        let address = document_address(&server.url(), "one");
        name_remote(&scratch, Some(&address), true);
        let (output, browser) = run(
            &scratch,
            &["remote", "login"],
            BrowserScenario::Visiting,
            None,
        );
        assert_eq!(output.status.code(), Some(1), "{length}: {output:?}");
        let said = stderr(&output);
        assert_eq!(
            said.contains("longer than 1048576 bytes"),
            longer,
            "{length}: {said}"
        );
        assert!(said.contains("/metadata"), "{length}: {said}");
        assert_eq!(browser.visits(), Vec::<String>::new(), "{length}");
    }
}
