//! The only writer of standard output and standard error; the crate denies `print!` and
//! `eprint!` everywhere else. A reader that stops reading early, such as `head -1`, closes
//! standard output: every later line to it is skipped in silence, and the command still exits
//! with its own code. Any other failure to write it is a failure the command does not expect: its
//! message goes to standard error once, and [`finish`] turns the exit code into 1 (RQ-154,
//! AC-579). A failure to write standard error has nowhere to be reported, and is ignored.

use std::fmt;
use std::io::{self, ErrorKind, Write};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};

/// Set once standard output can no longer be written: every later line to it is skipped.
static SKIPPED: AtomicBool = AtomicBool::new(false);

/// Set once writing standard output failed for a reason other than its closing.
static FAILED: AtomicBool = AtomicBool::new(false);

/// Prints a line on standard output, as `println!` does.
macro_rules! outln {
    ($($argument:tt)*) => {
        $crate::out::stdout_line(format_args!($($argument)*))
    };
}

/// Prints a line on standard error, as `eprintln!` does.
macro_rules! errln {
    ($($argument:tt)*) => {
        $crate::out::stderr_line(format_args!($($argument)*))
    };
}

pub(crate) use {errln, outln};

/// Writes `line` and a newline to standard output, unless it can no longer be written.
pub fn stdout_line(line: fmt::Arguments<'_>) {
    if SKIPPED.load(Ordering::Relaxed) {
        return;
    }
    let written = writeln!(io::stdout().lock(), "{line}");
    settle(written);
}

/// Writes `line` and a newline to standard error, ignoring a failure.
pub fn stderr_line(line: fmt::Arguments<'_>) {
    let _ = writeln!(io::stderr().lock(), "{line}");
}

/// Flushes standard output and gives the code the command exits with: `code`, or 1 when writing
/// standard output failed for a reason other than its closing.
pub fn finish(code: ExitCode) -> ExitCode {
    if !SKIPPED.load(Ordering::Relaxed) {
        settle(io::stdout().lock().flush());
    }
    if FAILED.load(Ordering::Relaxed) {
        ExitCode::FAILURE
    } else {
        code
    }
}

/// Records the result of a write to standard output: a closed output skips every later line in
/// silence; any other failure also reports itself and fails the command.
fn settle(written: io::Result<()>) {
    let Err(error) = written else {
        return;
    };
    SKIPPED.store(true, Ordering::Relaxed);
    if error.kind() != ErrorKind::BrokenPipe {
        FAILED.store(true, Ordering::Relaxed);
        errln!("cannot write to standard output: {error}");
    }
}
