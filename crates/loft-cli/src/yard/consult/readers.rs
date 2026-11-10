//! The classes of what a shell command's reading programs read: each reading program, the
//! options that take a value, the pattern a searcher takes first, `find`'s expression, git's
//! paths after `--`, redirections and `cd`, as `consult.ts` reads them.

use super::paths::{Class, class_of, ordered, resolve};
use super::words::{Word, assignments, commands_of};

/// How a reading program takes its words.
struct Reader {
    /// Options whose value is the next word, with the number of words each takes.
    values: &'static [(&'static str, usize)],
    /// Its first word that is not an option is a pattern, a script or a filter, not a path.
    pattern_first: bool,
    /// Options that give the pattern, after which the first word is a path.
    pattern_options: &'static [&'static str],
    /// Naming no path, it reads the directory it runs in rather than its standard input.
    searches: bool,
}

const ENDS: &[(&str, usize)] = &[("-n", 1), ("-c", 1)];
const GREP_VALUES: &[(&str, usize)] = &[
    ("-e", 1),
    ("-f", 1),
    ("-A", 1),
    ("-B", 1),
    ("-C", 1),
    ("-m", 1),
    ("--regexp", 1),
    ("--file", 1),
];
const GREP_PATTERNS: &[&str] = &["-e", "-f", "--regexp", "--file"];
const RG_VALUES: &[(&str, usize)] = &[
    ("-e", 1),
    ("-f", 1),
    ("-g", 1),
    ("-t", 1),
    ("-T", 1),
    ("-A", 1),
    ("-B", 1),
    ("-C", 1),
    ("-m", 1),
    ("-M", 1),
    ("-j", 1),
    ("--glob", 1),
    ("--type", 1),
    ("--type-not", 1),
    ("--regexp", 1),
    ("--file", 1),
];
const FD_VALUES: &[(&str, usize)] = &[
    ("-e", 1),
    ("-t", 1),
    ("-E", 1),
    ("-d", 1),
    ("--type", 1),
    ("--extension", 1),
    ("--exclude", 1),
    ("--max-depth", 1),
];

/// A reader that names its paths after its options, and reads its standard input without one.
const PLAIN: Reader = Reader {
    values: &[],
    pattern_first: false,
    pattern_options: &[],
    searches: false,
};

/// A grep: its pattern first, unless an option gives it.
const GREP: Reader = Reader {
    values: GREP_VALUES,
    pattern_first: true,
    pattern_options: GREP_PATTERNS,
    ..PLAIN
};

/// The reading programs, by name.
const READERS: &[(&str, Reader)] = &[
    ("cat", PLAIN),
    ("less", PLAIN),
    ("more", PLAIN),
    ("nl", PLAIN),
    ("wc", PLAIN),
    ("diff", PLAIN),
    (
        "head",
        Reader {
            values: ENDS,
            ..PLAIN
        },
    ),
    (
        "tail",
        Reader {
            values: ENDS,
            ..PLAIN
        },
    ),
    (
        "bat",
        Reader {
            values: &[("-l", 1), ("-r", 1), ("--language", 1), ("--line-range", 1)],
            ..PLAIN
        },
    ),
    (
        "cut",
        Reader {
            values: &[("-d", 1), ("-f", 1), ("-c", 1), ("-b", 1)],
            ..PLAIN
        },
    ),
    (
        "sort",
        Reader {
            values: &[("-k", 1), ("-t", 1), ("-o", 1)],
            ..PLAIN
        },
    ),
    (
        "uniq",
        Reader {
            values: &[("-f", 1), ("-s", 1), ("-w", 1)],
            ..PLAIN
        },
    ),
    (
        "sed",
        Reader {
            values: &[("-e", 1), ("-f", 1), ("--expression", 1), ("--file", 1)],
            pattern_first: true,
            pattern_options: &["-e", "-f", "--expression", "--file"],
            ..PLAIN
        },
    ),
    (
        "awk",
        Reader {
            values: &[("-F", 1), ("-v", 1), ("-f", 1)],
            pattern_first: true,
            pattern_options: &["-f"],
            ..PLAIN
        },
    ),
    ("grep", GREP),
    ("egrep", GREP),
    ("fgrep", GREP),
    (
        "rg",
        Reader {
            values: RG_VALUES,
            searches: true,
            ..GREP
        },
    ),
    (
        "ag",
        Reader {
            values: &[("-G", 1), ("-A", 1), ("-B", 1), ("-C", 1), ("-m", 1)],
            pattern_first: true,
            searches: true,
            ..PLAIN
        },
    ),
    (
        "fd",
        Reader {
            values: FD_VALUES,
            pattern_first: true,
            searches: true,
            ..PLAIN
        },
    ),
    (
        "ls",
        Reader {
            searches: true,
            ..PLAIN
        },
    ),
    (
        "tree",
        Reader {
            values: &[("-L", 1), ("-I", 1), ("-P", 1)],
            searches: true,
            ..PLAIN
        },
    ),
    (
        "jq",
        Reader {
            values: &[
                ("--arg", 2),
                ("--argjson", 2),
                ("-f", 1),
                ("--from-file", 1),
            ],
            pattern_first: true,
            pattern_options: &["-f", "--from-file"],
            ..PLAIN
        },
    ),
    (
        "yq",
        Reader {
            pattern_first: true,
            ..PLAIN
        },
    ),
];

/// The reading program named `program`, where it is one.
fn reader(program: &str) -> Option<&'static Reader> {
    READERS
        .iter()
        .find(|(name, _)| *name == program)
        .map(|(_, reader)| reader)
}

/// git's subcommands that read, whose paths are the words after `--`.
const GIT_READERS: &[&str] = &["diff", "show", "log", "grep"];

/// A directory no tree holds, for a command that moved to `~`.
const OUTSIDE: &str = "/~";

/// The words of a command that are not redirections, and the files its input is redirected
/// from: `>`, `>>`, `<`, `<<`, `<<<`, each after a descriptor or `&`, its target attached or next.
fn redirections(words: &[Word]) -> (Vec<&Word>, Vec<String>) {
    let mut kept = Vec::new();
    let mut inputs = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let word = &words[i];
        let redirected = (!word.quoted).then(|| redirection(&word.text)).flatten();
        let Some((operator, attached)) = redirected else {
            kept.push(word);
            i += 1;
            continue;
        };
        let target = if attached.is_empty() {
            i += 1;
            words.get(i).map(|word| word.text.clone())
        } else {
            Some(attached.to_owned())
        };
        if operator == "<"
            && let Some(target) = target
            && !target.starts_with('&')
        {
            inputs.push(target);
        }
        i += 1;
    }
    (kept, inputs)
}

/// The operator and attached target of a redirection word.
fn redirection(word: &str) -> Option<(&'static str, &str)> {
    let rest = word
        .strip_prefix('&')
        .unwrap_or_else(|| word.trim_start_matches(|c: char| c.is_ascii_digit()));
    ["<<<", "<<-", "<<", ">>", "<", ">"]
        .into_iter()
        .find_map(|operator| {
            let attached = rest.strip_prefix(operator)?;
            Some((if operator == "<<-" { "<<" } else { operator }, attached))
        })
}

/// The paths a reading program's arguments name.
fn reader_paths(reader: &Reader, args: &[&Word]) -> Vec<String> {
    let mut paths = Vec::new();
    let mut pattern_given = false;
    let mut options = true;
    let mut i = 0;
    while i < args.len() {
        let text = args[i].text.as_str();
        i += 1;
        if options && text == "--" {
            options = false;
            continue;
        }
        if options && text.starts_with('-') && text != "-" {
            if reader.pattern_options.contains(&text) {
                pattern_given = true;
            }
            i += reader
                .values
                .iter()
                .find(|(option, _)| *option == text)
                .map_or(0, |(_, words)| *words);
            continue;
        }
        if reader.pattern_first && !pattern_given {
            pattern_given = true;
            continue;
        }
        paths.push(text.to_owned());
    }
    paths
}

/// The paths `find` searches: its words before its expression.
fn find_paths(args: &[&Word]) -> Vec<String> {
    args.iter()
        .map(|word| word.text.as_str())
        .take_while(|text| !(text.starts_with('-') || matches!(*text, "(" | "\\(" | "!")))
        .map(str::to_owned)
        .collect()
}

/// The paths a git command reads and the directory it runs in, where it reads.
fn git_paths(args: &[&Word]) -> Option<(Vec<String>, Option<String>)> {
    let mut dir = None;
    let mut i = 0;
    while i < args.len() && args[i].text.starts_with('-') {
        match args[i].text.as_str() {
            "-C" => {
                i += 1;
                dir = args.get(i).map(|word| word.text.clone());
            }
            "-c" => i += 1,
            _ => {}
        }
        i += 1;
    }
    if !GIT_READERS.contains(&args.get(i)?.text.as_str()) {
        return None;
    }
    let paths = args
        .iter()
        .position(|word| word.text == "--")
        .map_or_else(Vec::new, |dashes| {
            args[dashes + 1..]
                .iter()
                .map(|word| word.text.clone())
                .collect()
        });
    Some((paths, dir))
}

/// The classes of what a shell command's reading programs read, run from `dir`.
pub fn shell_reads(command: &str, dir: &str, trees: &[String]) -> Vec<Class> {
    let mut classes = Vec::new();
    let mut here = dir.to_owned();
    for all in commands_of(command) {
        let (words, inputs) = redirections(&all[assignments(&all)..]);
        let Some((first, args)) = words.split_first() else {
            continue;
        };
        let program = first.text.rsplit('/').next().unwrap_or_default();
        if program == "cd" {
            let to = args.first().map_or("~", |word| word.text.as_str());
            here = resolve(to, &here).unwrap_or_else(|| OUTSIDE.to_owned());
            continue;
        }
        let mut from = here.clone();
        let (mut paths, searches) = match program {
            "git" => {
                let Some((paths, moved)) = git_paths(args) else {
                    continue;
                };
                if let Some(moved) = moved {
                    from = resolve(&moved, &here).unwrap_or_else(|| OUTSIDE.to_owned());
                }
                (paths, true)
            }
            "find" => (find_paths(args), true),
            program => {
                let Some(reader) = reader(program) else {
                    continue;
                };
                (reader_paths(reader, args), reader.searches)
            }
        };
        paths.extend(inputs);
        if paths.is_empty() && searches {
            paths.push(".".to_owned());
        }
        classes.extend(paths.iter().map(|path| class_of(path, &from, trees)));
    }
    ordered(classes)
}
