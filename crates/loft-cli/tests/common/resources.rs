//! What the contract suites of a remote's related resources share, of the simulator and of a
//! real remote alike: the member of the remote's protected resource metadata that lists them, the
//! list read from it, and the origin an address lies on.

use super::oauth::fetch;

/// The member of a remote's protected resource metadata that lists its related resources.
pub const RELATED_RESOURCES: &str = "related_resources";

/// The related resources that the protected resource metadata of the document at `address`
/// lists, found from the challenge of a request without a token: each as its `resource` and its
/// `kind`, in the order listed, and none where the metadata lists none.
///
/// # Panics
///
/// The member is not a list, or one of its entries holds no string `resource` or `kind`.
pub fn related_of(address: &str) -> Vec<(String, String)> {
    let (_, metadata, _) = fetch(address);
    let Some(listed) = metadata.get(RELATED_RESOURCES) else {
        return Vec::new();
    };
    let entries = listed
        .as_array()
        .unwrap_or_else(|| panic!("the related resources are listed in an array: {listed}"));
    entries
        .iter()
        .map(|entry| {
            let field = |name: &str| {
                entry[name]
                    .as_str()
                    .unwrap_or_else(|| panic!("an entry with a string `{name}`: {entry}"))
                    .to_owned()
            };
            (field("resource"), field("kind"))
        })
        .collect()
}

/// The origin of the absolute URL `url`, as RFC 6454 compares origins: its scheme and its host
/// in lower case, and its port where it is not the scheme's default.
pub fn origin_of(url: &str) -> String {
    let (scheme, rest) = url
        .split_once("://")
        .unwrap_or_else(|| panic!("an absolute URL: {url}"));
    let scheme = scheme.to_ascii_lowercase();
    let authority = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let default = match scheme.as_str() {
        "https" => ":443",
        "http" => ":80",
        _ => "",
    };
    let host_and_port = match authority.strip_suffix(default) {
        Some(host) if !default.is_empty() => host,
        _ => &authority,
    };
    format!("{scheme}://{host_and_port}")
}
