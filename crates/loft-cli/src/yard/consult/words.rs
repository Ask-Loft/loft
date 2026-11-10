//! A shell command split into simple commands and words, as the Claude mod's `shell.ts` splits
//! it, and the `loft` subcommands it runs (the Feature "Record a session's events"): `loft` by
//! its name, by a path to it, or through a shell variable it sets to either; never a word of
//! quoted text or of a heredoc.

use std::collections::HashMap;

/// `loft`'s own subcommands: a name is taken only from this list.
const LOFT_SUBCOMMANDS: &[&str] = &[
    "init",
    "fmt",
    "check",
    "import",
    "publish",
    "pull",
    "find",
    "terms",
    "reconcile",
    "verify",
    "status",
    "review",
    "merge-driver",
    "skill",
    "record-install",
    "remote",
    "telemetry",
    "update-check",
    "yard",
    "render",
];

/// A word of a shell command, and whether any of it was quoted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Word {
    pub text: String,
    pub quoted: bool,
}

/// The command with every heredoc's body left out.
fn without_heredocs(command: &str) -> String {
    let mut kept: Vec<&str> = Vec::new();
    let mut ending: Option<String> = None;
    for line in command.split('\n') {
        if let Some(end) = &ending {
            if line.trim() == end {
                ending = None;
            }
            continue;
        }
        kept.push(line);
        ending = heredoc_end(line);
    }
    kept.join("\n")
}

/// The word that ends the heredoc `line` opens, where it opens one: `<<END`, `<<-END`,
/// `<<'END'` or `<<"END"`, at the first `<<` that opens one, as `shell.ts`'s pattern finds it.
fn heredoc_end(line: &str) -> Option<String> {
    line.match_indices("<<")
        .find_map(|(at, _)| heredoc_at(&line[at + 2..]))
}

/// The word that ends a heredoc whose `<<` `rest` follows, where it opens one.
fn heredoc_at(rest: &str) -> Option<String> {
    let rest = rest.strip_prefix('-').unwrap_or(rest);
    let rest = rest.trim_start_matches([' ', '\t']);
    let (quote, rest) = match rest.chars().next() {
        Some(quote @ ('\'' | '"')) => (Some(quote), &rest[1..]),
        _ => (None, rest),
    };
    let name: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    let starts = name.chars().next()?;
    if starts.is_ascii_digit() {
        return None;
    }
    if let Some(quote) = quote
        && !rest[name.len()..].starts_with(quote)
    {
        return None;
    }
    Some(name)
}

/// Whether `inner`, the text between double quotes, is only a variable: `$NAME` or `${NAME}`.
fn is_variable(inner: &str) -> bool {
    variable_name(inner).is_some()
}

/// The name of the variable that `word` is alone: `$NAME` or `${NAME}`.
pub fn variable_name(word: &str) -> Option<&str> {
    let body = word.strip_prefix('$')?;
    let name = match body.strip_prefix('{') {
        Some(braced) => braced.strip_suffix('}')?,
        None => body,
    };
    let mut chars = name.chars();
    let first = chars.next()?;
    ((first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_'))
    .then_some(name)
}

/// The simple commands of a shell command, each as its words, with every heredoc's body left out.
pub fn commands_of(command: &str) -> Vec<Vec<Word>> {
    let source: Vec<char> = without_heredocs(command).chars().collect();
    let mut commands: Vec<Vec<Word>> = vec![Vec::new()];
    let mut word = String::new();
    let mut quoted = false;
    let mut is_word = false;
    let end = |commands: &mut Vec<Vec<Word>>,
               word: &mut String,
               quoted: &mut bool,
               is_word: &mut bool| {
        if *is_word {
            commands.last_mut().unwrap().push(Word {
                text: std::mem::take(word),
                quoted: *quoted,
            });
        }
        word.clear();
        *quoted = false;
        *is_word = false;
    };
    let mut i = 0;
    while i < source.len() {
        let c = source[i];
        if c == '\'' || c == '"' {
            let close = source[i + 1..]
                .iter()
                .position(|&d| d == c)
                .map(|at| i + 1 + at);
            let inner: String = source[i + 1..close.unwrap_or(source.len())]
                .iter()
                .collect();
            quoted = quoted || !(c == '"' && is_variable(&inner));
            word.push_str(&inner);
            is_word = true;
            i = close.unwrap_or(source.len());
        } else if c == ' ' || c == '\t' {
            end(&mut commands, &mut word, &mut quoted, &mut is_word);
        } else if matches!(c, ';' | '&' | '|' | '(' | ')' | '\n') {
            end(&mut commands, &mut word, &mut quoted, &mut is_word);
            if !commands.last().unwrap().is_empty() {
                commands.push(Vec::new());
            }
        } else {
            word.push(c);
            is_word = true;
        }
        i += 1;
    }
    end(&mut commands, &mut word, &mut quoted, &mut is_word);
    commands.retain(|words| !words.is_empty());
    commands
}

/// The name and value of `word` where it assigns a shell variable: `NAME=value`.
pub fn assignment(word: &str) -> Option<(&str, &str)> {
    let (name, value) = word.split_once('=')?;
    let mut chars = name.chars();
    let first = chars.next()?;
    ((first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_'))
    .then_some((name, value))
}

/// The number of words that open `words` by assigning shell variables.
pub fn assignments(words: &[Word]) -> usize {
    words
        .iter()
        .take_while(|word| !word.quoted && assignment(&word.text).is_some())
        .count()
}

fn is_loft(word: &str) -> bool {
    word == "loft"
        || ["/loft", "\\loft", "/loft.exe", "\\loft.exe"]
            .iter()
            .any(|end| word.ends_with(end))
}

/// The `loft` subcommands the shell command runs, each named once, in the order they run.
pub fn loft_subcommands(command: &str) -> Vec<String> {
    let mut variables: HashMap<String, String> = HashMap::new();
    let mut names: Vec<String> = Vec::new();
    for words in commands_of(command) {
        let n = assignments(&words);
        if n == words.len() {
            let assigned = words.iter().filter_map(|word| assignment(&word.text));
            variables.extend(assigned.map(|(name, value)| (name.to_owned(), value.to_owned())));
            continue;
        }
        let first = &words[n];
        let program = match variable_name(&first.text) {
            Some(name) => variables.get(name).cloned(),
            None => (!first.quoted).then(|| first.text.clone()),
        };
        if !program.is_some_and(|program| is_loft(&program)) {
            continue;
        }
        let sub = words[n + 1..]
            .iter()
            .find(|word| !word.text.starts_with('-'));
        if let Some(sub) = sub
            && !sub.quoted
            && LOFT_SUBCOMMANDS.contains(&sub.text.as_str())
            && !names.contains(&sub.text)
        {
            names.push(sub.text.clone());
        }
    }
    names
}
