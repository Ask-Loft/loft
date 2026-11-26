#!/bin/sh
# The Loft plugin's command hooks run `loft` through this script, with `loft`'s arguments as its
# own (the Feature "The Loft plugin"). A `loftCommand` setting other than `loft` is run as
# written. Otherwise it runs the `loft` on the search path, else the one `cargo install` puts in
# ~/.cargo/bin, which a host that starts Claude Code without reading the shell's startup files
# leaves off the search path. Where no `loft` can be run, the SessionStart hook tells the agent
# that Yard Client is off and how to turn it on, and every other hook prints nothing and exits 0,
# so that Claude Code reports no hook error.

# The setting's words, split as the mod splits them, with none meaning `loft`.
set -f
# shellcheck disable=SC2086
setting=$(printf '%s ' ${CLAUDE_PLUGIN_OPTION_LOFTCOMMAND:-loft})
set +f
setting=${setting% }
if [ -n "$setting" ] && [ "$setting" != loft ]; then
  # A setting of several words, such as `pnpm exec loft`, runs its first word with the others as
  # its first arguments.
  set -f
  # shellcheck disable=SC2086
  set -- $setting "$@"
  set +f
  if command -v "$1" >/dev/null 2>&1; then exec "$@"; fi
  tried="\`$setting\`"
else
  if command -v loft >/dev/null 2>&1; then exec loft "$@"; fi
  tried="\`loft\` on the search path"
  home=${HOME:-$USERPROFILE}
  for found in "$home/.cargo/bin/loft" "$home/.cargo/bin/loft.exe"; do
    if [ -f "$found" ] && [ -x "$found" ]; then exec "$found" "$@"; fi
    tried="$tried, \`$found\`"
  done
fi

# The hook object on standard input names the event; grep reads it all where it is not SessionStart.
if grep -E '"hook_event_name"[[:space:]]*:[[:space:]]*"SessionStart"' >/dev/null; then
  context="Yard Client is off in this session: the Loft plugin could not run loft. It tried $tried. The tools of the \`yard\` skill, \`mcp__loft__yard_read\` and the others, are not registered. Tell the person that Yard Client is off and how to turn it on: set the loft plugin's \`loftCommand\` setting to a command that runs loft, such as the absolute path of the \`loft\` binary, or put \`loft\` on the search path of the app that starts Claude Code, then start a new session."
  escaped=$(printf '%s' "$context" | tr '\t\r\n' '   ' | sed 's/\\/\\\\/g; s/"/\\"/g')
  printf '{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"%s"}}\n' "$escaped"
fi
exit 0
