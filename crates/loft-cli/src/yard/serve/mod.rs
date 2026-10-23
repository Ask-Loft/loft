//! `loft yard serve` (the Serve the page, Share one page server and Replace an older page server
//! Features): the page server, one on the machine. A run records its session as heard and its
//! tree as served; where a page server of this `loft`'s version or a newer one already runs, it
//! leaves it serving and exits; where an older one runs, it asks it to stop through
//! `~/.yard/stop.json` and waits for its lock to be freed. Then it takes the lock on
//! `~/.yard/server.lock`, which it holds while it runs, listens on the loopback interface, names
//! itself in `~/.yard/server.json`, and serves until a newer one asks it to stop.

mod activity;
mod assets;
mod discovery;
mod event;
mod events;
mod guard;
mod http;
mod listener;
mod live;
mod logs;
mod markdown;
mod routes;
mod session_files;
mod stream;
mod tokens;
mod trees;
mod watch;

use std::cmp::Ordering as Compared;
use std::collections::VecDeque;
use std::io;
use std::sync::atomic::Ordering;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use loft_ports::clock::Clock as _;
use serde_json::{Value, json};
use yardlog_store::home::{Home, HomeFile};
use yardlog_store::lock::{self, Locked};
use yardlog_store::session::is_session_id;

use super::args::ServeArgs;
use super::failure::Failure;
use super::page::SERVER_LOCK;
use super::{clock, now, protocol, sessions};
use crate::finish::Finished;
use crate::json;
use crate::out::outln;
use crate::version::VERSION;
use events::Sessions;
use listener::Listener;

const SCHEMA: &str = "loft-yard-serve/1";

/// The longest a run waits for another page server to stop, or to finish starting.
const WAITING: Duration = Duration::from_secs(10);

/// How often a run looks again while it waits.
const AGAIN: Duration = Duration::from_millis(100);

/// The running page server, which every connection's thread shares.
pub struct Server {
    port: u16,
    token: String,
    home: Home,
    /// How many changes the page server has seen to the files of the Yard Logs it serves.
    changes: AtomicU64,
    /// The latest changes, by their count, and the areas of the page each changed.
    recent: Mutex<VecDeque<(u64, Vec<live::Area>)>>,
    /// Whether it is stopping, for a newer one.
    stopping: AtomicBool,
    /// How many responses it is sending.
    busy: AtomicUsize,
    sessions: Sessions,
    /// The lock on `~/.yard/server.lock`, held for as long as the page server runs.
    _lock: Locked,
}

/// How many of the latest changes the page server keeps, for a page that falls behind.
const RECENT: usize = 64;

impl Server {
    /// Counts one change, of the areas `areas`.
    fn changed(&self, areas: Vec<live::Area>) {
        let mut recent = self
            .recent
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let count = self.changes.fetch_add(1, Ordering::SeqCst) + 1;
        recent.push_back((count, areas));
        while recent.len() > RECENT {
            recent.pop_front();
        }
    }

    /// The areas changed since the change `seen`, or none where the page server no longer keeps
    /// all of them.
    fn changed_since(&self, seen: u64) -> Option<Vec<live::Area>> {
        let recent = self
            .recent
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if recent.front().is_none_or(|(count, _)| *count > seen + 1) {
            return None;
        }
        let mut areas: Vec<live::Area> = recent
            .iter()
            .filter(|(count, _)| *count > seen)
            .flat_map(|(_, areas)| areas.iter().cloned())
            .collect();
        areas.sort();
        areas.dedup();
        Some(areas)
    }
}

/// What a run comes to: another page server runs, or this one serves.
enum Started {
    Running(Value),
    Serving(Listener, Arc<Server>, Value),
}

pub fn run(args: &ServeArgs) -> Finished {
    let as_json = args.output.json();
    match start(args) {
        Ok(Started::Running(fields)) => {
            report("running", fields, as_json);
            Finished::success("running")
        }
        Ok(Started::Serving(listener, server, fields)) => {
            report("serving", fields, as_json);
            let following = Arc::clone(&server);
            thread::spawn(move || live::follow(&following));
            listener.serve(move |connection| routes::handle(&server, connection));
            Failure::one("port", "the page server's socket failed").finish(SCHEMA, as_json)
        }
        Err(failure) => failure.finish(SCHEMA, as_json),
    }
}

fn report(outcome: &str, fields: Value, as_json: bool) {
    if as_json {
        json::print(&json::result(SCHEMA, outcome, fields, Vec::new()));
    } else {
        let verb = if outcome == "serving" {
            "serves"
        } else {
            "already serves"
        };
        outln!(
            "The page server of loft {} {verb} the page at {}",
            fields["version"].as_str().unwrap_or(VERSION),
            fields["url"].as_str().unwrap_or_default()
        );
    }
}

fn address(port: u16) -> String {
    format!("http://127.0.0.1:{port}/")
}

fn start(args: &ServeArgs) -> Result<Started, Failure> {
    let home = Home::from_env().ok_or_else(Failure::no_home)?;
    if let Some(session) = &args.session {
        if !is_session_id(session) {
            return Err(Failure::Usage(format!(
                "--session: `{session}` cannot name a session's file"
            )));
        }
        sessions::heard(&home, session, now()?)?;
    }
    if let Some(tree) = &args.tree {
        trees::register(&home, tree)?;
    }
    let lock = match take_over(&home)? {
        Ok(lock) => lock,
        Err(running) => {
            return Ok(Started::Running(json!({
                "port": running.port,
                "url": address(running.port),
                "version": running.version,
            })));
        }
    };
    let listener = listener::listen(args.port).map_err(|error| held(args.port, &error))?;
    super::page::serving(args.port);
    let token = token(&home)?;
    let formats = protocol::report()["formats"]["reads"].clone();
    let named = json!({
        "port": args.port,
        "pid": std::process::id(),
        "version": VERSION,
        "formats": formats,
    });
    home.write(HomeFile::Server, format!("{named:#}\n").as_bytes())?;
    home.remove(HomeFile::Stop)?;
    let server = Arc::new(Server {
        port: args.port,
        token,
        home,
        changes: AtomicU64::new(0),
        recent: Mutex::new(VecDeque::new()),
        stopping: AtomicBool::new(false),
        busy: AtomicUsize::new(0),
        sessions: Sessions::default(),
        _lock: lock,
    });
    let mut fields = named;
    fields["url"] = json!(address(args.port));
    Ok(Started::Serving(listener, server, fields))
}

/// The error a port that cannot be listened on gives.
fn held(port: u16, error: &io::Error) -> Failure {
    let message = if error.kind() == io::ErrorKind::AddrInUse {
        format!("port {port} is held by another program, not a page server")
    } else {
        format!("port {port} cannot be listened on: {error}")
    };
    Failure::one("port", message)
}

/// The lock on `~/.yard/server.lock`, once no page server holds it: an older one asked to stop
/// and waited for, or none; or the page server of this version or a newer one that runs.
fn take_over(home: &Home) -> Result<Result<Locked, discovery::Running>, Failure> {
    let clock = clock();
    let began = clock.elapsed();
    let mut asked = false;
    loop {
        if let Some(running) = discovery::running(home)? {
            if discovery::against_ours(&running.version) != Compared::Less {
                return Ok(Err(running));
            }
            if !asked {
                discovery::request_stop(home)?;
                asked = true;
            }
        } else if let Some(locked) = lock::try_lock(&home.dir().join(SERVER_LOCK))? {
            return Ok(Ok(locked));
        }
        if clock.elapsed().saturating_sub(began) >= WAITING {
            return Err(Failure::one(
                "stop",
                "another page server holds `~/.yard/server.lock` and did not give it up",
            ));
        }
        clock.wait(AGAIN);
    }
}

/// The control token in `~/.yard/token`, made where there is none.
fn token(home: &Home) -> Result<String, Failure> {
    if let Some(bytes) = home.read(HomeFile::Token)? {
        let token = String::from_utf8_lossy(&bytes).trim().to_owned();
        if !token.is_empty() {
            return Ok(token);
        }
    }
    let token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    home.write(HomeFile::Token, token.as_bytes())?;
    Ok(token)
}
