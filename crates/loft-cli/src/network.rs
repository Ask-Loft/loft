//! The network client, the only part of `loft` that opens a network connection. It runs the
//! work a command leaves for the network after the command's own work is done, all of it within
//! one time limit, and never writes to standard error: a service that refuses, fails or stays
//! silent changes nothing a person sees. It also makes the calls to a connected document's
//! remote that a command exists to make, each under the time limit its caller gives, and reads
//! an address's origin, and whether two addresses lie on one, as the parser its requests use
//! reads them.

use std::fmt;
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

/// An adapter's authenticated post to the local Yard page server, with no redirects or Origin.
pub fn post_local_json(port: u16, path: &str, token: &str, body: &[u8]) -> bool {
    if token.is_empty() || !matches!(path, "/api/events" | "/api/heard" | "/api/register") {
        return false;
    }
    agent(LIMIT, "loft-yard")
        .post(format!("http://127.0.0.1:{port}{path}"))
        .header("Content-Type", "application/json")
        .header("Authorization", &format!("Bearer {token}"))
        .send(body)
        .is_ok_and(|response| response.status().is_success())
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

/// The most a body read from a remote may hold.
const BODY_LIMIT: u64 = 256 << 20;

/// An answer as it came: its status, the headers a caller reads, and its body.
#[derive(Debug, Clone)]
pub struct Reply {
    pub status: u16,
    pub retry_after: Option<String>,
    /// Every `WWW-Authenticate` header, in the order given.
    pub www_authenticate: Vec<String>,
    pub body: Vec<u8>,
}

/// `method` of `url` with `headers` and `body`, within `timeout`, following no redirect, for a
/// call a command exists to make: its answer read whole, or why none came.
///
/// # Errors
///
/// `url` may not be contacted, or no answer came in full within `timeout`.
pub fn call(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: Option<&[u8]>,
    timeout: Duration,
) -> Result<Reply, String> {
    let uri = allowed(url).ok_or_else(|| format!("{url}: {CONTACTED}"))?;
    let mut request = ureq::http::Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let agent = agent(timeout, &format!("loft/{VERSION}"));
    let response = match body {
        Some(body) => agent.run(request.body(body).map_err(|e| e.to_string())?),
        None => agent.run(request.body(()).map_err(|e| e.to_string())?),
    };
    let mut response = response.map_err(|error| error.to_string())?;
    let header = |name: &str| {
        response
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
    };
    let retry_after = header("retry-after");
    let www_authenticate = response
        .headers()
        .get_all("www-authenticate")
        .iter()
        .filter_map(|value| value.to_str().ok().map(str::to_owned))
        .collect();
    let status = response.status().as_u16();
    let body = response
        .body_mut()
        .with_config()
        .limit(BODY_LIMIT)
        .read_to_vec()
        .map_err(|error| error.to_string())?;
    Ok(Reply {
        status,
        retry_after,
        www_authenticate,
        body,
    })
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

/// What the network client contacts, as a refusal of another address says it.
pub const CONTACTED: &str =
    "only an https address, or an http one on the loopback interface, is contacted";

/// `url` as the address the request goes to, if it may be contacted: any `https` address, and
/// an `http` one only on the loopback interface, where nothing crosses a network. The address
/// is parsed once, by the parser the request uses, so the host checked is the host contacted.
pub fn allowed(url: &str) -> Option<Uri> {
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

/// The origin of an absolute address, as RFC 6454 compares origins: its scheme and its host in
/// lower case, and its port where the address names one other than the scheme's default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    scheme: String,
    host: String,
    port: Option<u16>,
}

impl fmt::Display for Origin {
    /// The origin written as an address: `<scheme>://<host>`, then `:<port>` where it has one.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}://{}", self.scheme, self.host)?;
        match self.port {
            Some(port) => write!(formatter, ":{port}"),
            None => Ok(()),
        }
    }
}

/// `url` as the parser the request uses reads it, if it is an absolute address with a host: its
/// origin, and its path, `/` where it names none, without the query that may follow. A fragment
/// is never read. The one reader of an origin in `loft`, so that the origin read is the origin
/// contacted. The parser also reads a scheme that is empty or does not begin with a letter, which
/// RFC 3986, section 3.1, does not allow, and a host that is empty, which names no host: such a
/// text is read as none.
#[must_use]
pub fn origin_and_path(url: &str) -> Option<(Origin, String)> {
    read(url).map(|(origin, uri)| (origin, uri.path().to_owned()))
}

/// `url` as [`origin_and_path`] reads it, with its query: its origin, its path, and its query
/// with the `?` that opens it, or nothing where it has none. A fragment is never read.
#[must_use]
pub fn origin_path_and_query(url: &str) -> Option<(Origin, String, String)> {
    read(url).map(|(origin, uri)| {
        let query = uri
            .query()
            .map_or_else(String::new, |query| format!("?{query}"));
        (origin, uri.path().to_owned(), query)
    })
}

/// `url` as an origin alone, as [`origin_and_path`] reads it: an absolute address with a host,
/// whose path is `/` or none, with no query and no fragment. The parser leaves a fragment out, so
/// the text is looked at for one.
#[must_use]
pub fn origin_alone(url: &str) -> Option<Origin> {
    if url.contains('#') {
        return None;
    }
    let (origin, uri) = read(url)?;
    (uri.path() == "/" && uri.query().is_none()).then_some(origin)
}

/// `url` as [`origin_and_path`] reads it: its origin and the parsed address.
fn read(url: &str) -> Option<(Origin, Uri)> {
    let uri: Uri = url.parse().ok()?;
    let scheme = uri
        .scheme_str()
        .filter(|scheme| scheme.starts_with(|first: char| first.is_ascii_alphabetic()))?
        .to_ascii_lowercase();
    let default = match scheme.as_str() {
        "https" => Some(443),
        "http" => Some(80),
        _ => None,
    };
    let origin = Origin {
        host: uri
            .host()
            .filter(|host| !host.is_empty())?
            .to_ascii_lowercase(),
        port: uri.port_u16().filter(|port| Some(*port) != default),
        scheme,
    };
    Some((origin, uri))
}

/// Whether `one` and `other` lie on one origin, as RFC 6454 compares origins: the same scheme,
/// the same host without regard to case, and the same port, the scheme's default where an
/// address names none. Both are read by the parser the request uses, so the origin compared is
/// the origin contacted. An address that is not an absolute `https` or `http` one lies on no
/// origin, not even its own.
#[must_use]
pub fn same_origin(one: &str, other: &str) -> bool {
    matches!((origin(one), origin(other)), (Some(one), Some(other)) if one == other)
}

/// The origin of `url`, as [`origin_and_path`] reads it, or none for an address that is not an
/// absolute `https` or `http` one.
#[must_use]
pub fn origin(url: &str) -> Option<Origin> {
    read(url)
        .map(|(origin, _)| origin)
        .filter(|origin| matches!(origin.scheme.as_str(), "https" | "http"))
}

#[cfg(test)]
mod tests {
    use super::{allowed, origin, origin_and_path, origin_path_and_query, same_origin};

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

    #[test]
    fn addresses_with_one_scheme_host_and_effective_port_lie_on_one_origin() {
        let address = "https://remote.example/documents/one";
        for other in [
            "https://remote.example/.well-known/oauth-protected-resource/documents",
            "https://REMOTE.Example/x",
            "HTTPS://remote.example/x",
            "https://remote.example:443/x",
            "https://someone@remote.example/x?y=z",
        ] {
            assert!(same_origin(address, other), "{other}");
            assert!(same_origin(other, address), "{other}");
        }
        assert!(same_origin("http://127.0.0.1/a", "http://127.0.0.1:80/b"));
        assert!(same_origin("http://[::1]:8080/a", "http://[::1]:8080/b"));
    }

    #[test]
    fn an_address_of_another_scheme_host_or_port_lies_on_another_origin() {
        let address = "https://remote.example/documents/one";
        for other in [
            "http://remote.example/x",
            "https://elsewhere.example/x",
            "https://notes.remote.example/x",
            "https://remote.example@elsewhere.example/x",
            "https://remote.example:8443/x",
            "https://remote.example./x",
            // An address that is not an absolute `https` or `http` one lies on no origin.
            "/.well-known/oauth-protected-resource/documents",
            "ftp://remote.example/x",
            "not an address",
        ] {
            assert!(!same_origin(address, other), "{other}");
            assert!(!same_origin(other, address), "{other}");
        }
        for (one, other) in [
            ("http://127.0.0.1:8080/a", "http://127.0.0.1:8081/b"),
            ("http://127.0.0.1:8080/a", "http://localhost:8080/b"),
            // Two spellings of one IPv6 address are two hosts, as RFC 6454 compares them.
            ("http://[::1]:8080/a", "http://[0:0:0:0:0:0:0:1]:8080/b"),
        ] {
            assert!(!same_origin(one, other), "{one} {other}");
        }
        assert!(!same_origin("not an address", "not an address"));
    }

    #[test]
    fn an_address_is_read_as_its_origin_written_whole_and_its_path() {
        for (url, written, path) in [
            (
                "HTTPS://Remote.Example:443/Documents/one?a=b#c",
                "https://remote.example",
                "/Documents/one",
            ),
            (
                "https://someone@remote.example:8443",
                "https://remote.example:8443",
                "/",
            ),
            ("http://[::1]:80/a/", "http://[::1]", "/a/"),
            // An address of another scheme is read too, so that a caller can name its origin.
            ("ftp://Files.Example/a", "ftp://files.example", "/a"),
        ] {
            let (origin, read) = origin_and_path(url).unwrap_or_else(|| panic!("{url}"));
            assert_eq!(
                (origin.to_string().as_str(), read.as_str()),
                (written, path)
            );
        }
        for url in [
            "remote.example/a",
            "/documents",
            "https://",
            "not an address",
            "https://exa mple.org",
            // A scheme begins with a letter (RFC 3986, section 3.1), and a host is not empty.
            "://remote.example/a",
            "1https://remote.example/a",
            "https://:443/a",
        ] {
            assert!(origin_and_path(url).is_none(), "{url}");
        }
        // Only an `https` or `http` address lies on an origin that is compared.
        assert!(origin("ftp://files.example/a").is_none());
    }

    #[test]
    fn an_address_is_read_as_its_origin_its_path_and_its_query() {
        for (url, written, path, query) in [
            (
                "HTTPS://Remote.Example:443/mail/inbox?view=all#top",
                "https://remote.example",
                "/mail/inbox",
                "?view=all",
            ),
            (
                "https://remote.example/notes/",
                "https://remote.example",
                "/notes/",
                "",
            ),
            (
                "http://127.0.0.1:4000?view=all",
                "http://127.0.0.1:4000",
                "/",
                "?view=all",
            ),
        ] {
            let (origin, read, asked) =
                origin_path_and_query(url).unwrap_or_else(|| panic!("{url}"));
            assert_eq!(
                (origin.to_string().as_str(), read.as_str(), asked.as_str()),
                (written, path, query),
                "{url}"
            );
        }
        for url in [
            "/notes?view=all",
            "not an address",
            "://remote.example/a?b=c",
        ] {
            assert!(origin_path_and_query(url).is_none(), "{url}");
        }
    }

    #[test]
    fn an_address_with_user_information_lies_on_the_origin_of_its_host_and_port() {
        // The user information names one loopback port, and the host and port another.
        let resource = "http://127.0.0.1:4000@127.0.0.1:5000/documents";
        assert!(origin(resource).is_some(), "{resource}");
        assert_eq!(origin(resource), origin("http://127.0.0.1:5000/"));
        assert_ne!(origin(resource), origin("http://127.0.0.1:4000/"));
        assert!(!same_origin(resource, "http://127.0.0.1:4000/documents/a"));
        assert!(same_origin(resource, "http://127.0.0.1:5000/documents/a"));
    }
}
