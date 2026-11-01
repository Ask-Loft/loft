//! The browser simulator: a program that a test names in `BROWSER`, which the binary under test
//! runs with the address it would open in the person's browser. A visiting scenario requests the
//! address and follows each redirect, as a person who consents does, so a sign-in comes back to
//! the binary's loopback port; an absent one requests nothing and fails, as a browser that
//! cannot be opened does. Each address it is given, and the arguments of every run, are recorded
//! for the test to read; a run with other than one argument requests nothing and exits 2.

use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::install::browser_executable;

/// The variable that names the scenario's directory to the simulator.
pub const SCENARIO_ENV: &str = "SOFTWARE_PLANT_BROWSER";
/// The file of the scenario's directory that says how the browser behaves.
const BEHAVIOUR_FILE: &str = "browser-scenario";
/// The file of the scenario's directory that lists the arguments of each run, one run a line,
/// as a JSON array.
const ARGUMENTS_FILE: &str = "browser-arguments";
/// The most redirects a visit follows.
const HOPS: usize = 10;

/// How the simulated browser behaves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrowserScenario {
    /// Requests the address and follows each redirect.
    Visiting,
    /// Requests nothing and exits 1.
    Absent,
}

/// A browser scenario written into a directory.
#[derive(Debug, Clone)]
pub struct InstalledBrowser {
    dir: PathBuf,
}

impl BrowserScenario {
    /// Writes the scenario into `dir`, which then holds its record of visits.
    #[must_use]
    pub fn install(self, dir: &Path) -> InstalledBrowser {
        let behaviour = match self {
            Self::Visiting => "visiting",
            Self::Absent => "absent",
        };
        fs::write(dir.join(BEHAVIOUR_FILE), behaviour).expect("the browser scenario is written");
        let _ = fs::remove_file(dir.join(ARGUMENTS_FILE));
        InstalledBrowser {
            dir: dir.to_path_buf(),
        }
    }
}

impl InstalledBrowser {
    /// Names the simulator in `BROWSER` for `command`, with its scenario.
    pub fn apply<'a>(&self, command: &'a mut Command) -> &'a mut Command {
        command
            .env("BROWSER", browser_executable())
            .env(SCENARIO_ENV, &self.dir)
    }

    /// The arguments of each run of the browser, in order.
    ///
    /// # Panics
    ///
    /// A run's record is not a JSON array of strings.
    #[must_use]
    pub fn arguments(&self) -> Vec<Vec<String>> {
        fs::read_to_string(self.dir.join(ARGUMENTS_FILE))
            .map(|text| {
                text.lines()
                    .map(|line| {
                        serde_json::from_str(line).expect("each run's arguments are a JSON array")
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Every address the browser was given, in order: the argument of each run with one.
    #[must_use]
    pub fn visits(&self) -> Vec<String> {
        self.arguments()
            .into_iter()
            .filter_map(|run| <[String; 1]>::try_from(run).ok())
            .map(|[address]| address)
            .collect()
    }
}

/// Runs the simulator with `arguments`, as its scenario in `dir` says, and gives its exit code:
/// 2 for other than one argument, the address.
#[must_use]
pub fn run(dir: &Path, arguments: &[String]) -> u8 {
    let runs = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(ARGUMENTS_FILE));
    if let Ok(mut runs) = runs {
        let _ = writeln!(runs, "{}", serde_json::json!(arguments));
    }
    let [address] = arguments else {
        return 2;
    };
    match fs::read_to_string(dir.join(BEHAVIOUR_FILE)).as_deref() {
        Ok("visiting") => u8::from(visit(address).is_err()),
        _ => 1,
    }
}

/// Requests `address` and follows each redirect, until an answer that is not one.
fn visit(address: &str) -> io::Result<()> {
    let mut next = address.to_owned();
    for _ in 0..HOPS {
        match get(&next)? {
            Some(location) if location.starts_with('/') => {
                next = format!("{}{location}", origin(&next));
            }
            Some(location) => next = location,
            None => return Ok(()),
        }
    }
    Err(io::Error::other("too many redirects"))
}

/// The scheme and authority of `address`, against which a redirect to a path is resolved.
fn origin(address: &str) -> &str {
    let after = address.find("://").map_or(0, |at| at + 3);
    let end = address[after..]
        .find('/')
        .map_or(address.len(), |at| after + at);
    &address[..end]
}

/// `GET address` over plain HTTP, giving the `Location` of a redirect.
fn get(address: &str) -> io::Result<Option<String>> {
    let rest = address
        .strip_prefix("http://")
        .ok_or_else(|| io::Error::other(format!("not an http address: {address}")))?;
    let (authority, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
    let path = if path.is_empty() { "/" } else { path };
    let mut stream = TcpStream::connect(authority)?;
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n\r\n"
    )?;
    let mut reader = BufReader::new(stream);
    let mut status = String::new();
    reader.read_line(&mut status)?;
    let redirect = status
        .split_whitespace()
        .nth(1)
        .is_some_and(|code| code.starts_with('3'));
    let mut location = None;
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 || line.trim_end().is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.trim().eq_ignore_ascii_case("location")
        {
            location = Some(value.trim().to_owned());
        }
    }
    Ok(if redirect { location } else { None })
}
