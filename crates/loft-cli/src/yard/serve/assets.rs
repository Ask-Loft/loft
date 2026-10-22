//! The page's files, built into `loft` from `crates/loft-cli/yard-page/` and served as they are
//! kept there: the page is browser-native, with no build step, so the files `loft` was built with
//! are the files the browser runs. Each file is listed once here, by the path the page server
//! serves it at.

/// One file of the page.
pub struct Asset {
    /// The path the page server serves it at, without its leading `/`.
    pub path: &'static str,
    /// Its media type.
    pub kind: &'static str,
    pub body: &'static [u8],
}

/// The file the page server serves at `/`.
pub const INDEX: &str = "index.html";

/// Every file of the page.
pub const ASSETS: &[Asset] = &[
    Asset {
        path: INDEX,
        kind: "text/html; charset=utf-8",
        body: include_bytes!("../../../yard-page/index.html"),
    },
    Asset {
        path: "yard.css",
        kind: "text/css; charset=utf-8",
        body: include_bytes!("../../../yard-page/yard.css"),
    },
];

/// The file of the page served at `path`, `/` naming the page itself.
pub fn find(path: &str) -> Option<&'static Asset> {
    let path = path.trim_start_matches('/');
    let path = if path.is_empty() { INDEX } else { path };
    ASSETS.iter().find(|asset| asset.path == path)
}
