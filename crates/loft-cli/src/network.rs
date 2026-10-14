//! The network client, the only part of `loft` that opens a network connection. It runs the
//! work a command leaves for the network after the command's own work is done, all of it within
//! one time limit, and never writes to standard error: a service that refuses, fails or stays
//! silent changes nothing a person sees.

use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use ureq::Agent;

use crate::version::VERSION;

/// The most the network client adds to a command run.
pub const LIMIT: Duration = Duration::from_millis(500);

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
    if !allowed(url) {
        return None;
    }
    let response = agent(left, 0)
        .post(url)
        .header("Content-Type", "application/json")
        .send(body)
        .ok()?;
    Some(response.status().as_u16())
}

fn agent(left: Duration, redirects: u32) -> Agent {
    Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(left))
        .max_redirects(redirects)
        .user_agent(format!("loft/{VERSION}"))
        .build()
        .into()
}

/// Whether `url` may be contacted: any `https` address, and an `http` one only on the loopback
/// interface, where nothing crosses a network.
pub fn allowed(url: &str) -> bool {
    if url.starts_with("https://") {
        return true;
    }
    let Some(rest) = url.strip_prefix("http://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = match authority.strip_prefix('[') {
        Some(bracketed) => bracketed.split(']').next().unwrap_or_default(),
        None => authority
            .rsplit_once(':')
            .map_or(authority, |(host, _)| host),
    };
    host == "localhost"
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|address| address.is_loopback())
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
            assert!(allowed(url), "{url}");
        }
        for url in [
            "http://askloft.ai/api/telemetry",
            "http://10.0.0.5:8080",
            "http://localhost.example.com/",
            "http://[2001:db8::1]:80/",
            "ftp://127.0.0.1/",
            "127.0.0.1:9",
        ] {
            assert!(!allowed(url), "{url}");
        }
    }
}
