# `loft` on Claude Code

Use this workflow for running the `loft` command line on Claude Code: through which tool, how to
read its output, and how to report progress. Every rule of the specification lives in `SKILL.md`
and the other references; this file records only what is particular to this host. Never restate
or override them here.

## Contents

- Run `loft`
- Read its output
- Report progress
- Delegate

## Run `loft`

Run `loft` with the Bash tool from the directory that holds `spec/`. `loft --version`,
`loft check`, `loft fmt --check` and `loft verify` change nothing and run without asking; every
other command writes and asks first. Edit the files of `spec/` with the Edit and Write tools,
never with shell redirection.

## Read its output

Pass `--format json` when you will act on the result: the document goes to standard output and
messages to standard error. A large document is best read through a filter, such as
`loft check --format json | jq '.findings[] | select(.severity == "warning")'`, rather than whole.

## Report progress

Say in a line what you are about to change before a write to `spec/`, and summarise the changed
nodes after each run of the loop. Report findings you leave in place with their reason.

## Delegate

A review of a large specification can be split by Product or Feature Group across subagents,
each told which references to read and to change nothing. Merge their findings into one report
ordered by severity.
