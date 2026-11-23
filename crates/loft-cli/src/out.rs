//! The only writer of standard output and standard error; the crate denies `print!` and
//! `eprint!` everywhere else. A reader that stops reading early, such as `head -1`, closes
//! standard output: every later line to it is skipped in silence, and the command still exits
//! with its own code. Any other failure to write it is a failure the command does not expect: its
//! message goes to standard error once, and [`finish`] turns the exit code into 1 (RQ-154,
//! AC-579). A failure to write standard error has nowhere to be reported, and is ignored.

use std::borrow::Cow;
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

/// Writes `line` and a newline to standard error, ignoring a failure. Standard error is read by
/// a person at a terminal, and a line may quote what a remote or an authorization server
/// answered, so each control character but the line feed and the tab is written as U+FFFD:
/// nothing a line quotes can move the cursor, clear the screen or rename the window.
pub fn stderr_line(line: fmt::Arguments<'_>) {
    let line = line.to_string();
    let _ = writeln!(io::stderr().lock(), "{}", printable(&line));
}

/// `text` with each control character but the line feed and the tab replaced by U+FFFD: every
/// character in U+0000 to U+001F but those two, U+007F, and U+0080 to U+009F.
fn printable(text: &str) -> Cow<'_, str> {
    let unprintable = |character: char| character.is_control() && !matches!(character, '\n' | '\t');
    if text.contains(unprintable) {
        Cow::Owned(text.replace(unprintable, "\u{fffd}"))
    } else {
        Cow::Borrowed(text)
    }
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

#[cfg(test)]
mod tests {
    use super::printable;

    #[test]
    fn each_control_character_but_the_line_feed_and_the_tab_is_written_as_the_replacement_character()
     {
        let controls = (0..=0x1f_u32)
            .chain(0x7f..=0x9f)
            .filter_map(char::from_u32)
            .filter(|character| !matches!(character, '\n' | '\t'));
        for control in controls {
            let written = printable(&format!("a{control}b")).into_owned();
            assert_eq!(written, "a\u{fffd}b", "U+{:04X}", u32::from(control));
        }
    }

    #[test]
    fn every_other_character_is_written_as_it_is() {
        let text = "a line\n\tand another: ~ \u{a0} é 語 \u{1f600} \u{fffd}";
        assert_eq!(printable(text), text);
    }
}
