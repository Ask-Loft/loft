//! The only writer of standard output and standard error; the crate denies `print!` and
//! `eprint!` everywhere else. A reader that stops reading early, such as `head -1`, closes
//! standard output: every later line to it is skipped in silence, and the command still exits
//! with its own code. Any other failure to write it is a failure the command does not expect: its
//! message goes to standard error once, and [`finish`] turns the exit code into 1 (RQ-154,
//! AC-579). A failure to write standard error has nowhere to be reported, and is ignored.

use std::fmt;
use std::io::{self, ErrorKind, Write};
use std::process::ExitCode;
use std::sync::atomic::{AtomicU8, Ordering};

/// What became of standard output: [`OPEN`], [`CLOSED`] or [`FAILED`].
static STDOUT: AtomicU8 = AtomicU8::new(OPEN);

/// Standard output can be written.
const OPEN: u8 = 0;

/// Its reader closed standard output: every later line to it is skipped.
const CLOSED: u8 = 1;

/// Writing standard output failed for a reason other than its closing: every later line to it is
/// skipped, and the command fails.
const FAILED: u8 = 2;

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
    if STDOUT.load(Ordering::Relaxed) != OPEN {
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
    if STDOUT.load(Ordering::Relaxed) == OPEN {
        settle(io::stdout().lock().flush());
    }
    if STDOUT.load(Ordering::Relaxed) == FAILED {
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
    if error.kind() == ErrorKind::BrokenPipe {
        STDOUT.store(CLOSED, Ordering::Relaxed);
    } else {
        STDOUT.store(FAILED, Ordering::Relaxed);
        errln!("cannot write to standard output: {error}");
    }
}
