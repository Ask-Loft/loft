//! The network client, the only part of `loft` that opens a network connection. It runs the
//! work a command leaves for the network after the command's own work is done, all of it within
//! one time limit, and never writes to standard error: a service that refuses, fails or stays
//! silent changes nothing a person sees.

use std::net::IpAddr;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use ureq::Agent;
use ureq::http::Uri;

use crate::version::VERSION;

/// The most the network client adds to a command run.
const LIMIT: Duration = Duration::from_millis(500);

/// Work for the network, given the time it has left.
pub type Job = Box<dyn FnOnce(Duration) + Send>;

/// Runs `jobs` side by side and returns once each has finished or [`LIMIT`] has passed. A job
/// still running then is abandoned, and the process ends without it.
pub fn run(jobs: Vec<Job>) {
    if jobs.is_empty() {
        return;
    }
    let deadline = Instant::now() + LIMIT;
    let (done, finished) = mpsc::channel();
    let count = jobs.len();
    for job in jobs {
        let done = done.clone();
        thread::spawn(move || {
            job(deadline.saturating_duration_since(Instant::now()));
            let _ = done.send(());
        });
    }
    for _ in 0..count {
        let left = deadline.saturating_duration_since(Instant::now());
        if finished.recv_timeout(left).is_err() {
            return;
        }
    }
}

/// `POST`s `body` as JSON to `url` within `left`, following no redirect, and gives the
/// answer's status, or none when no answer came.
pub fn post_json(url: &str, body: &[u8], left: Duration) -> Option<u16> {
    let response = agent(left, &format!("loft/{VERSION}"))
        .post(allowed(url)?)
        .header("Content-Type", "application/json")
        .send(body)
        .ok()?;
    Some(response.status().as_u16())
}

/// `GET`s `url` within `left`, asking for GitHub's JSON, following no redirect, and gives the
/// answer's status and body, or none when no answer came. The request names `loft` and nothing
/// else about it, not even its version.
pub fn get_json(url: &str, left: Duration) -> Option<(u16, Vec<u8>)> {
    let mut response = agent(left, "loft")
        .get(allowed(url)?)
        .header("Accept", "application/vnd.github+json")
        .call()
        .ok()?;
    let status = response.status().as_u16();
    let body = response
        .body_mut()
        .with_config()
        .limit(1 << 20)
        .read_to_vec()
        .ok()?;
    Some((status, body))
}

/// A client that gives up after `left`, follows no redirect, since a redirect could lead off
/// the loopback interface over plain `http`, and names itself `user_agent`.
fn agent(left: Duration, user_agent: &str) -> Agent {
    Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(left))
        .max_redirects(0)
        .user_agent(user_agent)
        .build()
        .into()
}

/// `url` as the address the request goes to, if it may be contacted: any `https` address, and
/// an `http` one only on the loopback interface, where nothing crosses a network. The address
/// is parsed once, by the parser the request uses, so the host checked is the host contacted.
fn allowed(url: &str) -> Option<Uri> {
    let uri: Uri = url.parse().ok()?;
    let host = uri.host()?.trim_start_matches('[').trim_end_matches(']');
    let loopback = host == "localhost"
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback());
    match uri.scheme_str()? {
        "https" => Some(uri),
        "http" if loopback => Some(uri),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::allowed;

    #[test]
    fn https_is_allowed_anywhere_and_http_only_on_the_loopback_interface() {
        for url in [
            "https://askloft.ai/api/telemetry",
            "https://api.github.com",
            "http://127.0.0.1:9/api/telemetry",
            "http://127.0.0.2:8080",
            "http://localhost:3000/x",
            "http://[::1]:8080/",
        ] {
            assert!(allowed(url).is_some(), "{url}");
        }
        for url in [
            "http://askloft.ai/api/telemetry",
            "http://10.0.0.5:8080",
            "http://localhost.example.com/",
            "http://[2001:db8::1]:80/",
            "ftp://127.0.0.1/",
            "127.0.0.1:9",
            "http://127.0.0.1:9@askloft.ai/api/telemetry",
            "http://localhost@10.0.0.5/",
        ] {
            assert!(allowed(url).is_none(), "{url}");
        }
    }
}
