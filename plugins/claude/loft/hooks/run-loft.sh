#!/bin/sh
# The Loft plugin's command hooks run `loft` through this script, with `loft`'s arguments as its
# own (the Feature "The Loft plugin"). A `loftCommand` setting other than `loft` is run as
# written. Otherwise it runs the `loft` on the search path, else the one `cargo install` puts in
# ~/.cargo/bin, which a host that starts Claude Code without reading the shell's startup files
# leaves off the search path. Where no `loft` can be run, the SessionStart hook tells the agent
# that Yard Client is off and how to turn it on, and every other hook prints nothing and exits 0,
# so that Claude Code reports no hook error.

setting=${CLAUDE_PLUGIN_OPTION_LOFTCOMMAND:-loft}
if [ "$setting" != loft ]; then
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
  for found in "$HOME/.cargo/bin/loft" "$HOME/.cargo/bin/loft.exe"; do
    if [ -f "$found" ] && [ -x "$found" ]; then exec "$found" "$@"; fi
  done
  tried="\`loft\` on the search path, \`$HOME/.cargo/bin/loft\` and \`$HOME/.cargo/bin/loft.exe\`"
fi

hook=$(cat)
if printf '%s' "$hook" | grep -Eq '"hook_event_name"[[:space:]]*:[[:space:]]*"SessionStart"'; then
  context="Yard Client is off in this session: the Loft plugin could not run loft. It tried $tried. The tools of the \`yard\` skill, \`mcp__loft__yard_read\` and the others, are not registered. Tell the person that Yard Client is off and how to turn it on: set the loft plugin's \`loftCommand\` setting to a command that runs loft, such as the absolute path of the \`loft\` binary, or put \`loft\` on the search path of the app that starts Claude Code, then start a new session."
  escaped=$(printf '%s' "$context" | sed 's/\\/\\\\/g; s/"/\\"/g')
  printf '{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"%s"}}\n' "$escaped"
fi
exit 0
