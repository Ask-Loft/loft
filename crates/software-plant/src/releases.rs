//! The release feed simulator: an HTTP endpoint on the loopback interface that answers as
//! GitHub's REST API does for the releases of `Ask-Loft/loft`, without a token. A test points
//! the binary under test at it with `LOFT_UPDATE_ENDPOINT`, scripts the releases it lists, and
//! counts the requests the binary made.

use serde_json::json;

use crate::http::{Request, Responder, Response, Server, refusing_url};

/// The path of the latest release, under the API's address.
pub const LATEST: &str = "/repos/Ask-Loft/loft/releases/latest";

/// A release as the feed lists it.
#[derive(Clone, Debug)]
pub struct Release {
    pub tag: String,
    pub draft: bool,
    pub prerelease: bool,
}

impl Release {
    /// A published release tagged `tag`.
    #[must_use]
    pub fn published(tag: &str) -> Self {
        Self {
            tag: tag.to_owned(),
            draft: false,
            prerelease: false,
        }
    }

    /// A draft tagged `tag`.
    #[must_use]
    pub fn draft(tag: &str) -> Self {
        Self {
            draft: true,
            ..Self::published(tag)
        }
    }

    /// A prerelease tagged `tag`.
    #[must_use]
    pub fn prerelease(tag: &str) -> Self {
        Self {
            prerelease: true,
            ..Self::published(tag)
        }
    }

    /// The release's page on GitHub.
    #[must_use]
    pub fn page(&self) -> String {
        format!("https://github.com/Ask-Loft/loft/releases/tag/{}", self.tag)
    }

    fn json(&self) -> serde_json::Value {
        json!({
            "tag_name": self.tag,
            "name": self.tag,
            "html_url": self.page(),
            "draft": self.draft,
            "prerelease": self.prerelease,
        })
    }
}

/// How the feed answers.
#[derive(Clone, Debug)]
enum Behaviour {
    /// As GitHub does: the newest release that is neither a draft nor a prerelease.
    Listing(Vec<Release>),
    /// The latest release answered as exactly this one, as a mirror that misreports might.
    Answering(Release),
    Failing(u16),
    RateLimited,
    Silent,
}

/// A running release feed simulator.
pub struct ReleaseFeed {
    server: Server,
}

impl ReleaseFeed {
    /// A feed listing `releases`, newest first, that answers the latest release as GitHub
    /// does: the newest that is neither a draft nor a prerelease, or `404` when there is none.
    #[must_use]
    pub fn listing(releases: Vec<Release>) -> Self {
        Self::start(Behaviour::Listing(releases))
    }

    /// A feed that answers the latest release as exactly `release`, whatever its flags.
    #[must_use]
    pub fn answering(release: Release) -> Self {
        Self::start(Behaviour::Answering(release))
    }

    /// A feed that answers every request with `status`.
    #[must_use]
    pub fn failing(status: u16) -> Self {
        Self::start(Behaviour::Failing(status))
    }

    /// A feed whose limit for requests without a token is spent, as GitHub answers then.
    #[must_use]
    pub fn rate_limited() -> Self {
        Self::start(Behaviour::RateLimited)
    }

    /// A feed that accepts each connection and never answers.
    #[must_use]
    pub fn silent() -> Self {
        Self::start(Behaviour::Silent)
    }

    /// The address of a feed that refuses every connection.
    #[must_use]
    pub fn refusing() -> String {
        refusing_url()
    }

    fn start(behaviour: Behaviour) -> Self {
        Self {
            server: Server::start(Feed(behaviour)),
        }
    }

    /// The API's address, for `LOFT_UPDATE_ENDPOINT`.
    #[must_use]
    pub fn url(&self) -> String {
        self.server.url()
    }

    /// Every request received, answered or not.
    #[must_use]
    pub fn requests(&self) -> Vec<Request> {
        self.server.requests()
    }
}

struct Feed(Behaviour);

impl Responder for Feed {
    fn answer(&mut self, request: &Request) -> Option<Response> {
        match &self.0 {
            Behaviour::Silent => None,
            Behaviour::Failing(status) => Some(Response::empty(*status)),
            Behaviour::RateLimited => Some(rate_limited()),
            Behaviour::Answering(release) => Some(latest(request, Some(release))),
            Behaviour::Listing(releases) => {
                let latest_published = releases
                    .iter()
                    .find(|release| !release.draft && !release.prerelease);
                Some(latest(request, latest_published))
            }
        }
    }
}

/// The answer to `request` when `release` is the latest.
fn latest(request: &Request, release: Option<&Release>) -> Response {
    if request.method != "GET" {
        return Response::empty(404);
    }
    match (request.path.as_str(), release) {
        (LATEST, Some(release)) => Response::json(200, &release.json()),
        _ => Response::json(
            404,
            &json!({
                "message": "Not Found",
                "documentation_url": "https://docs.github.com/rest",
                "status": "404",
            }),
        ),
    }
}

/// GitHub's answer once the limit for requests without a token is spent.
fn rate_limited() -> Response {
    let mut response = Response::json(
        403,
        &json!({
            "message": "API rate limit exceeded.",
            "documentation_url": "https://docs.github.com/rest/overview/rate-limits-for-the-rest-api",
        }),
    );
    response.headers.extend([
        ("x-ratelimit-limit".to_owned(), "60".to_owned()),
        ("x-ratelimit-remaining".to_owned(), "0".to_owned()),
    ]);
    response
}
