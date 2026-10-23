//! The page's files, built into `loft` from `crates/loft-cli/yard-page/` and served as they are
//! kept there: the page is browser-native, with no build step, so the files `loft` was built with
//! are the files the browser runs. Each file is listed once here, by the path the page server
//! serves it at. A file kept compressed is served as it is kept, its encoding named.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;

/// One file of the page.
pub struct Asset {
    /// The path the page server serves it at, without its leading `/`.
    pub path: &'static str,
    /// Its media type.
    pub kind: &'static str,
    /// The content coding `body` is kept in, if it is kept compressed.
    pub encoding: Option<&'static str>,
    pub body: &'static [u8],
}

/// The file the page server serves at `/`.
pub const INDEX: &str = "index.html";

const HTML: &str = "text/html; charset=utf-8";
const CSS: &str = "text/css; charset=utf-8";
const JS: &str = "text/javascript; charset=utf-8";
const TEXT: &str = "text/plain; charset=utf-8";
const FONT: &str = "font/woff2";

/// The page's file at `path` under `yard-page/`, of the media type `kind`; or, with `gzip`, the
/// file kept as `path.gz` and served at `path`.
macro_rules! asset {
    ($path:literal, $kind:expr) => {
        Asset {
            path: $path,
            kind: $kind,
            encoding: None,
            body: include_bytes!(concat!("../../../yard-page/", $path)),
        }
    };
    ($path:literal, $kind:expr, gzip) => {
        Asset {
            path: $path,
            kind: $kind,
            encoding: Some("gzip"),
            body: include_bytes!(concat!("../../../yard-page/", $path, ".gz")),
        }
    };
}

/// Every file of the page.
pub const ASSETS: &[Asset] = &[
    asset!("index.html", HTML),
    asset!("yard.css", CSS),
    asset!("app.js", JS),
    asset!("lib/api.js", JS),
    asset!("lib/diagrams.js", JS),
    asset!("lib/dom.js", JS),
    asset!("lib/format.js", JS),
    asset!("lib/kept.js", JS),
    asset!("lib/modal.js", JS),
    asset!("lib/prefs.js", JS),
    asset!("lib/route.js", JS),
    asset!("model/answers.js", JS),
    asset!("model/change.js", JS),
    asset!("model/gantt.js", JS),
    asset!("model/log.js", JS),
    asset!("model/tokens.js", JS),
    asset!("views/answering.js", JS),
    asset!("views/blocks.js", JS),
    asset!("views/gantt.js", JS),
    asset!("views/inherited.js", JS),
    asset!("views/log.js", JS),
    asset!("views/timeline.js", JS),
    asset!("views/tokens.js", JS),
    asset!("views/trees.js", JS),
    asset!("views/loft/decision.js", JS),
    asset!("views/loft/findings.js", JS),
    asset!("views/loft/index.js", JS),
    asset!("views/loft/publish.js", JS),
    asset!("views/loft/spec-change.js", JS),
    asset!("views/loft/trace.js", JS),
    asset!("views/loft/trace-list.js", JS),
    // Mermaid, which draws the page's diagrams, as its release ships it, kept compressed, with its
    // licence; `vendor/README.md` names the release.
    asset!("vendor/mermaid.min.js", JS, gzip),
    asset!("vendor/mermaid.LICENSE", TEXT),
    // Inter, the Loft website's typeface, in its variable Latin cut, with its licence.
    asset!("vendor/inter-latin.woff2", FONT),
    asset!("vendor/inter.LICENSE", TEXT),
];

impl Asset {
    /// The file's entity tag: a digest of the bytes it is built from, worked out once.
    pub fn tag(&self) -> &'static str {
        static TAGS: OnceLock<Vec<String>> = OnceLock::new();
        let tags = TAGS.get_or_init(|| {
            ASSETS
                .iter()
                .map(|asset| {
                    let mut hasher = DefaultHasher::new();
                    asset.body.hash(&mut hasher);
                    format!("\"{:016x}\"", hasher.finish())
                })
                .collect()
        });
        let at = ASSETS
            .iter()
            .position(|asset| asset.path == self.path)
            .expect("an asset is one of the page's files");
        &tags[at]
    }
}

/// The file of the page served at `path`, `/` naming the page itself.
pub fn find(path: &str) -> Option<&'static Asset> {
    let path = path.trim_start_matches('/');
    let path = if path.is_empty() { INDEX } else { path };
    ASSETS.iter().find(|asset| asset.path == path)
}
