//! What the page server answers, by method and path:
//!
//! - `GET /` and `GET /<file>`: the page's files.
//! - `GET /api/version`: the page server's version and formats, as `loft yard protocol` gives
//!   them.
//! - `GET /api/trees`: every tree served and its branches' Yard Logs, and the open submissions.
//! - `GET /api/log?tree=<dir>&branch=<name>[&base=<branch>]`: a branch's Yard Log.
//! - `GET /api/stream`: the stream of changes.
//! - `POST /api/answer`: the person's answers and decisions, `{ tree, branch, answers }`.
//! - `POST /api/register`: a tree to serve, `{ tree }`.
//! - `POST /api/events`: one of the mod's events, or a list of them.
//!
//! Each `POST` changes something, so it is taken only from the page or with the control token.

use std::sync::atomic::Ordering;

use serde_json::{Value, json};

use super::Server;
use super::assets;
use super::guard::{Access, admit};
use super::http::{self, Request, Response};
use super::listener::Connection;
use super::logs;
use super::stream::stream;
use super::trees;
use crate::yard::failure::Failure;
use crate::yard::{protocol, sessions};
use yardlog_store::session::is_session_id;

/// Answers the one request `connection` carries.
pub fn handle(server: &Server, mut connection: Connection) {
    let request = match http::read(&mut connection) {
        Ok(Some(request)) => request,
        Ok(None) => return,
        Err(message) => {
            let _ = http::write(
                &mut connection,
                &Response::refused(400, "request", &message),
            );
            return;
        }
    };
    if request.method == "GET" && request.path == "/api/stream" {
        match admit(&request, Access::Read, server.port, &server.token) {
            Ok(()) => {
                connection.wait_without_limit();
                stream(server, connection);
            }
            Err(refused) => {
                let _ = http::write(&mut connection, &refused);
            }
        }
        return;
    }
    server.busy.fetch_add(1, Ordering::SeqCst);
    let response = if server.stopping.load(Ordering::SeqCst) {
        Response::refused(503, "stopping", "the page server is stopping")
    } else {
        respond(server, &request)
    };
    let _ = http::write(&mut connection, &response);
    server.busy.fetch_sub(1, Ordering::SeqCst);
}

fn respond(server: &Server, request: &Request) -> Response {
    let access = if request.method == "POST" {
        Access::Change
    } else {
        Access::Read
    };
    if let Err(refused) = admit(request, access, server.port, &server.token) {
        return refused;
    }
    let home = &server.home;
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/api/version") => Response::json(200, &protocol::report()),
        ("GET", "/api/trees") => match trees::listing(home, server.port, &server.entries) {
            Ok(listing) => Response::json(200, &listing),
            Err(failure) => failed(failure),
        },
        ("GET", "/api/log") => logs::read(
            home,
            request.param("tree"),
            request.param("branch"),
            server.port,
        ),
        ("POST", "/api/answer") => logs::answer(home, &request.body, server.port),
        ("POST", "/api/register") => register(server, &request.body),
        ("POST", "/api/heard") => heard(server, &request.body),
        ("POST", "/api/events") => server.sessions.record(home, &request.body),
        ("GET", path) if !path.starts_with("/api/") => match assets::find(path) {
            Some(asset) => Response::file(
                asset.kind,
                asset.encoding,
                asset.body,
                asset.tag(),
                request.header("if-none-match"),
            ),
            None => Response::refused(404, "path", &format!("the page has no file {path}")),
        },
        (_, path) => Response::refused(
            404,
            "path",
            &format!("the page server answers no {} {path}", request.method),
        ),
    }
}

fn register(server: &Server, body: &[u8]) -> Response {
    let sent: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
    let Some(tree) = sent["tree"].as_str() else {
        return Response::refused(400, "tree", "the request names no tree");
    };
    match trees::register(&server.home, std::path::Path::new(tree)) {
        Ok(tree) => {
            server.changes.fetch_add(1, Ordering::SeqCst);
            Response::json(200, &json!({ "tree": tree.display().to_string() }))
        }
        Err(failure) => failed(failure),
    }
}

/// Records that the session a mod names is live: its beat, every 5 seconds.
fn heard(server: &Server, body: &[u8]) -> Response {
    let sent: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
    let Some(session) = sent["session"]
        .as_str()
        .filter(|session| is_session_id(session))
    else {
        return Response::refused(400, "session", "the request names no session");
    };
    let heard = crate::yard::now().and_then(|at| {
        sessions::heard(&server.home, session, at)?;
        Ok(at)
    });
    match heard {
        Ok(at) => Response::json(200, &json!({ "session": session, "heard": at.to_string() })),
        Err(failure) => failed(failure),
    }
}

fn failed(failure: Failure) -> Response {
    Response::json(409, &json!({ "errors": trees::failure_value(failure) }))
}
