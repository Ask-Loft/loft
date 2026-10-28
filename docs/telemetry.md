# Telemetry and the update check

## What is sent

Telemetry is on by default. At the end of each run of `check`, `find`, `fmt`, `import v1`,
`import openspec`, `init`, `publish`, `reconcile`, `render`, `skill install`, `terms` and
`verify`, `loft` sends one command event to `https://askloft.ai/api/telemetry`, so that the
maintainers learn which commands are used, how they end and how long they take. Before the first
event leaves a machine, `loft` prints a notice on standard error saying what it sends and how to
switch it off. An event looks like this:

```json
{
  "schema": "loft-telemetry-event/2",
  "eventId": "0b9a3c0e-7f4d-4a43-9a52-3f3f2c4b8d11",
  "version": "2.3.0",
  "os": "macos",
  "arch": "aarch64",
  "installedBy": "npm",
  "time": "2026-10-14T15:00:00Z",
  "installation": "6f1d2c3b-9a8e-4f7d-8c6b-5a4e3d2c1b0a",
  "project": "1a2b3c4d-5e6f-4a7b-8c9d-0e1f2a3b4c5d",
  "branch": "9f86d081884c7d65",
  "mode": "agent",
  "agent": "claude-code",
  "command": "check",
  "outcome": "failed",
  "errorKinds": [],
  "duration": "under-1s"
}
```

`schemas/telemetry-event.json` lists every field and every value it may take. The time is the
hour, in UTC; `mode` says whether the run was internal, in CI, by a coding agent, at a terminal or
none of these; `agent` names the agent from the variables its host sets. The three tokens are
random: `installation` is drawn once for the user on the machine, `project` once for a checkout,
in a `.loft/` beside `spec/` that git ignores, and `branch` is the branch name keyed with a secret
kept in that same file, so the same branch in two checkouts gives two different tokens. An event
never carries the specification, the code, a path, a repository address, a name or an error
message. The tokens are pseudonymous rather than anonymous, and the collector sees the network
address each event comes from. Events with tokens are kept for at most 30 days, and aggregates
without them for at most 90 days.

Sending never gets in the way. It starts after the command's output, gives up after 500
milliseconds and prints nothing; a collector that is down, slow or refusing changes no output and
no exit code, and a lost event is never sent again.

## The update check

At the end of the same commands, at most once a day, `loft` asks GitHub for the latest release
and keeps the answer; a later run tells you on standard error, at most once a day, when a newer
release exists and how to install it. The request carries nothing about you or your project: its
User-Agent is `loft`, without even the version, and it follows no redirect. The check has its own
switch and stays on when telemetry is off.

The npm package records that npm installed it when it installs. After `cargo install`, or after
unpacking a binary from a GitHub release, `loft record-install cargo` or
`loft record-install release-binary` records the same, so that the update notice names the right
way to update; without it, `loft` infers the way from where it lies.

## Switches

| To                                   | Run or set                                   |
| ------------------------------------ | -------------------------------------------- |
| switch telemetry off for the user    | `loft telemetry off` (`on` and `status` too) |
| switch telemetry off for one shell   | `LOFT_TELEMETRY=off` (any value but `internal`) |
| switch the update check off          | `loft update-check off`, or `LOFT_UPDATE_CHECK=off` |
| send elsewhere                       | `LOFT_TELEMETRY_ENDPOINT=<url>`              |
| ask elsewhere for releases           | `LOFT_UPDATE_ENDPOINT=<GitHub API base>`     |

`loft telemetry off` deletes the installation token; a variable set to `off` wins over a
recorded `on`, and a recorded `off` wins over any variable. The settings live in `loft/` under
the user's configuration directory (`~/.config` on Linux, `~/Library/Application Support` on
macOS, `%APPDATA%` on Windows); when they cannot be written, `loft` does not check for updates at
all, rather than ask and tell again on every run. Neither address accepts plain `http` except on
the loopback interface. Both requests go through the proxy named by the first of `ALL_PROXY`,
`HTTPS_PROXY` and `HTTP_PROXY` that is set, unless `NO_PROXY` lists the host.
`LOFT_TELEMETRY=internal` marks runs by Loft's own maintainers.

The Claude Code plugin runs `loft` with both telemetry and the update check off.
