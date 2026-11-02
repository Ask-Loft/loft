//! `loft skill install --server <origin>`: the skill package that a server publishes, such as a
//! connected document's remote, installed into a skill directory of its own beside `loft-spec`.
//! The server's manifest names the package and its digest and length; the package is checked
//! against them, and each of its entries against its own directory, before anything is written,
//! and no plugin is enabled (the accepted decision "A server's skill is installed beside
//! `loft-spec`, checked before it is written").

use std::io::Read;
use std::path::{Component, Path};

use flate2::read::GzDecoder;
use serde_json::{Value, json};

use super::{Hosts, SCHEMA, write};
use crate::finish::Finished;
use crate::json;
use crate::network;
use crate::out::outln;
use crate::remote;
use crate::remote::sha256_hex;
use crate::stop::Stop;

/// Where a server serves its skill manifest, from its origin.
const MANIFEST: &str = "/.well-known/loft-skill.json";

/// The most a package may hold once uncompressed, so that a small package cannot fill the disk.
const UNPACKED_LIMIT: u64 = 64 << 20;

/// A package's files, each by its path from the top of the skill directory, with its bytes.
type Files = Vec<(String, Vec<u8>)>;

/// The skill a server's manifest names.
struct Manifest {
    name: String,
    package: String,
    sha256: String,
    length: u64,
}

/// `loft skill install --server <origin>`, its origin as [`origin_of`] gave it.
pub fn run(root: &Path, origin: &str, hosts: Hosts, as_json: bool) -> Finished {
    match install(root, origin, hosts) {
        Ok(written) => {
            if as_json {
                let fields = json!({ "skills": written });
                json::print(&json::result(SCHEMA, "installed", fields, Vec::new()));
            } else {
                for directory in &written {
                    outln!("installed {directory}");
                }
                let written: Vec<&str> = written.iter().map(String::as_str).collect();
                super::ask_to_commit(&written);
            }
            Finished::success("installed")
        }
        Err(message) => Stop::one("install", message).exit(SCHEMA, as_json),
    }
}

/// `text` as an origin: a scheme and an authority, with no path but `/`, no query and no
/// fragment, without its trailing `/`.
///
/// # Errors
///
/// `text` is not an origin; the message names it.
pub fn origin_of(text: &str) -> Result<String, String> {
    let not_an_origin =
        || format!("`--server {text}`: a server is named by its origin, a scheme and a host alone");
    let (scheme, rest) = text.split_once("://").ok_or_else(not_an_origin)?;
    let (authority, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
    if scheme.is_empty() || authority.is_empty() || !matches!(path, "" | "/") {
        return Err(not_an_origin());
    }
    if text.contains(['?', '#']) {
        return Err(not_an_origin());
    }
    Ok(format!("{scheme}://{authority}"))
}

/// Reads the manifest at `origin`, fetches and checks the package, and writes it into the skill
/// directory of each host `hosts` selects, naming each directory written.
fn install(root: &Path, origin: &str, hosts: Hosts) -> Result<Vec<String>, String> {
    if network::allowed(origin).is_none() {
        return Err(format!(
            "{origin}: a server is reached over `https`, or `http` on the loopback interface"
        ));
    }
    let timeout = remote::timeout().map_err(|error| error.to_string())?;
    let fetch = |path: &str| {
        let url = format!("{origin}{path}");
        match network::call("GET", &url, &[], None, timeout) {
            Ok(reply) if reply.status == 200 => Ok(reply.body),
            Ok(reply) => Err(format!("{url} answered {}", reply.status)),
            Err(error) => Err(error),
        }
    };
    let manifest = fetch(MANIFEST)
        .and_then(|body| manifest(&body))
        .map_err(|reason| format!("{origin}: no skill manifest reads: {reason}"))?;
    let package = fetch(&manifest.package).map_err(|reason| format!("{origin}: {reason}"))?;
    checked(&manifest, &package)?;
    let files = unpacked(&package, UNPACKED_LIMIT)?;
    hosts
        .selected()
        .iter()
        .map(|host| {
            let directory = format!("{}/{}", host.skills(), manifest.name);
            write::replace(&root.join(&directory), &files)
                .map_err(|error| format!("{directory}: {error}"))?;
            Ok(directory)
        })
        .collect()
}

/// The manifest in `body`.
///
/// # Errors
///
/// `body` is not a JSON object holding a string `name`, `version`, `package` and `sha256` and a
/// whole number `length`, its `package` is not a path on the same origin, or its `name` is not
/// a slug or is `loft-spec`.
fn manifest(body: &[u8]) -> Result<Manifest, String> {
    let value: Value = serde_json::from_slice(body).map_err(|error| error.to_string())?;
    let text = |field: &str| {
        value[field]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("it holds no string `{field}`"))
    };
    text("version")?;
    let manifest = Manifest {
        name: text("name")?,
        package: text("package")?,
        sha256: text("sha256")?,
        length: value["length"]
            .as_u64()
            .ok_or("it holds no whole number `length`")?,
    };
    if !manifest.package.starts_with('/') || manifest.package.starts_with("//") {
        return Err(format!(
            "its `package`, {}, is not a path on the same origin",
            manifest.package
        ));
    }
    let slug = !manifest.name.is_empty()
        && manifest
            .name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
    if !slug || manifest.name == "loft-spec" {
        return Err(format!(
            "its `name`, `{}`, is not a slug of lowercase letters, digits and hyphens other than \
             `loft-spec`",
            manifest.name
        ));
    }
    Ok(manifest)
}

/// Refuses a package whose length or SHA-256 digest is not the one the manifest gives.
fn checked(manifest: &Manifest, package: &[u8]) -> Result<(), String> {
    let digest = sha256_hex(package);
    let length = package.len() as u64;
    if digest.eq_ignore_ascii_case(&manifest.sha256) && length == manifest.length {
        return Ok(());
    }
    Err(format!(
        "the package is not the one the manifest names: the manifest gives SHA-256 {} and {} \
         bytes, the package received has SHA-256 {digest} and {length} bytes",
        manifest.sha256, manifest.length
    ))
}

/// The files of `package`, a gzip-compressed tar archive, each by its path from the top of the
/// skill directory.
///
/// # Errors
///
/// The package does not read or unpacks to more than `limit` bytes, or an entry lies outside the
/// skill directory: an absolute path, a `..` segment, a link, or neither a file nor a directory.
/// The message names the entry.
fn unpacked(package: &[u8], limit: u64) -> Result<Files, String> {
    let mut archive = tar::Archive::new(GzDecoder::new(package).take(limit));
    let files = files_of(&mut archive);
    // The limit cuts the stream short, so a package that reaches it is refused, whatever the
    // archive cut short reads as, rather than installed in part.
    if archive.into_inner().limit() == 0 {
        return Err(format!("the package unpacks to more than {limit} bytes"));
    }
    files
}

/// The files of `archive`, each by its path from the top of the skill directory.
///
/// # Errors
///
/// As [`unpacked`], but for its limit.
fn files_of(archive: &mut tar::Archive<impl Read>) -> Result<Files, String> {
    let unreadable = |error: std::io::Error| format!("the package does not read: {error}");
    let mut files = Vec::new();
    for entry in archive.entries().map_err(unreadable)? {
        let mut entry = entry.map_err(unreadable)?;
        let kind = entry.header().entry_type();
        // A pax global header, such as `git archive` writes first, describes the archive alone.
        if kind == tar::EntryType::XGlobalHeader {
            continue;
        }
        let path = entry.path().map_err(unreadable)?.into_owned();
        let shown = path.display().to_string();
        let outside = |why: &str| format!("the package's entry {shown} {why}");
        if kind.is_symlink() || kind.is_hard_link() {
            return Err(outside("is a link"));
        }
        if !kind.is_file() && !kind.is_dir() {
            return Err(outside("is neither a file nor a directory"));
        }
        let mut parts = Vec::new();
        for component in path.components() {
            match component {
                Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
                Component::CurDir => {}
                Component::ParentDir => return Err(outside("has a `..` segment")),
                Component::RootDir | Component::Prefix(_) => {
                    return Err(outside("has an absolute path"));
                }
            }
        }
        if kind.is_file() && !parts.is_empty() {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).map_err(unreadable)?;
            files.push((parts.join("/"), bytes));
        }
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use flate2::Compression;
    use flate2::write::GzEncoder;
    use tar::{Builder, EntryType, Header};

    use super::{origin_of, unpacked};

    /// A gzip-compressed tar archive holding a pax global header, then `files`.
    fn package(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut builder = Builder::new(GzEncoder::new(Vec::new(), Compression::default()));
        let global = b"30 comment=a commit of a repo\n";
        let mut header = Header::new_ustar();
        header.set_path("pax_global_header").unwrap();
        header.set_entry_type(EntryType::XGlobalHeader);
        header.set_size(global.len() as u64);
        header.set_cksum();
        builder.append(&header, &global[..]).unwrap();
        for (path, bytes) in files {
            let mut header = Header::new_ustar();
            header.set_path(path).unwrap();
            header.set_size(bytes.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder.append(&header, *bytes).unwrap();
        }
        let mut encoder = builder.into_inner().unwrap();
        encoder.flush().unwrap();
        encoder.finish().unwrap()
    }

    #[test]
    fn a_pax_global_header_is_passed_over_and_a_package_past_the_limit_is_refused() {
        let skill = package(&[("SKILL.md", b"a skill"), ("references/a.md", &[b'a'; 4096])]);
        let files = unpacked(&skill, 1 << 20).unwrap();
        let paths: Vec<&str> = files.iter().map(|(path, _)| path.as_str()).collect();
        assert_eq!(paths, ["SKILL.md", "references/a.md"]);
        let error = unpacked(&skill, 4096).unwrap_err();
        assert!(error.contains("more than 4096 bytes"), "{error}");
    }

    #[test]
    fn an_origin_is_a_scheme_and_a_host_alone() {
        assert_eq!(
            origin_of("https://example.org/").as_deref(),
            Ok("https://example.org")
        );
        assert_eq!(
            origin_of("http://127.0.0.1:8080").as_deref(),
            Ok("http://127.0.0.1:8080")
        );
        for refused in [
            "https://example.org/skills",
            "https://example.org/?q=1",
            "https://example.org#top",
            "example.org",
        ] {
            assert!(origin_of(refused).is_err(), "{refused}");
        }
    }
}
