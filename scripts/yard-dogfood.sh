#!/bin/zsh
# Start Claude Code or Codex with this worktree's Loft binary and Yard plugin.
# Each launch builds Loft and restarts the dogfood Yard server on port 7412, kept in
# ~/.yard-dogfood/ through LOFT_YARD_HOME; the installed loft's server in ~/.yard/ keeps running.
# Existing sessions reconnect; refresh open pages after a same-version UI rebuild.
# Both start in the caller's working directory. Requires Python 3.
# Claude is the default.
#   ./scripts/yard-dogfood.sh claude                    start Claude Code
#   ./scripts/yard-dogfood.sh claude resume <session-id> resume Claude Code
#   ./scripts/yard-dogfood.sh codex                     start Codex
#   ./scripts/yard-dogfood.sh codex resume <session-id>  resume Codex
# Claude Desktop takes no command-line flags, so one project at a time is switched instead:
#   ./scripts/yard-dogfood.sh desktop <project-dir>         its sessions run this worktree's plugin
#   ./scripts/yard-dogfood.sh desktop --undo <project-dir>  back to the installed plugin
set -euo pipefail

TREE=${0:A:h:h}
PORT=7412
LOFT="$TREE/target/release/loft"
WRAPPER="$TREE/.loft/yard-dogfood/bin/loft"
AGENT_COMMAND=claude
if [[ ${1-} == codex || ${1-} == claude || ${1-} == desktop ]]; then
  AGENT_COMMAND=$1
  shift
fi

if [[ $AGENT_COMMAND == desktop ]]; then
  DESKTOP=(python3 "$TREE/scripts/yard-dogfood-desktop.py" "$TREE" "$PORT" "$WRAPPER")
  if [[ ${1-} == --undo && $# -eq 2 && -n ${2-} ]]; then
    exec "${DESKTOP[@]}" "$2" --undo
  fi
  if [[ $# -ne 1 || -z ${1-} || ${1-} == -* ]]; then
    print -u2 -- "Usage: ${0:t} desktop [--undo] PROJECT-DIR"
    exit 2
  fi
fi

if [[ ${1-} == resume ]]; then
  if [[ $# -lt 2 || -z ${2-} || ${2-} == -* ]]; then
    print -u2 -- "Usage: ${0:t} {claude|codex} [resume SESSION] [agent options...]"
    exit 2
  fi
  if [[ $AGENT_COMMAND == claude ]]; then
    shift
    set -- --resume "$@"
  fi
fi

python3 "$TREE/scripts/yard-dogfood-server.py" "$TREE" "$PORT"

# The session's loft: this worktree's build, keeping its machine home in ~/.yard-dogfood/.
DOGFOOD_HOME="$HOME/.yard-dogfood"
mkdir -p "${WRAPPER:h}"
print -r -- "#!/bin/sh
LOFT_YARD_HOME=${(qq)DOGFOOD_HOME}
export LOFT_YARD_HOME
exec ${(qq)LOFT} \"\$@\"" > "$WRAPPER"
chmod 755 "$WRAPPER"

if [[ $AGENT_COMMAND == desktop ]]; then
  exec "${DESKTOP[@]}" "$1"
fi

echo "Yard page: http://127.0.0.1:$PORT/"
if [[ $AGENT_COMMAND == codex ]]; then
  # Generate a local package from the repo source, with this binary and port in its hooks.
  # Codex installs it under a separate identity so it does not replace the released plugin.
  CODEX_MARKETPLACE=$(python3 - "$TREE" "$PORT" "$WRAPPER" <<'PY'
import json
from pathlib import Path
import shlex
import shutil
import sys

tree = Path(sys.argv[1])
market = tree / ".loft/yard-dogfood/codex"
package = market / "plugin"
if package.exists():
    shutil.rmtree(package)
shutil.copytree(tree / "plugins/codex/loft", package)
hooks_path = package / "hooks/hooks.json"
hooks = json.loads(hooks_path.read_text())
for groups in hooks["hooks"].values():
    for group in groups:
        for hook in group["hooks"]:
            words = shlex.split(hook["command"])
            if words[:3] != ["loft", "yard", "codex"]:
                raise ValueError("Unexpected Codex hook command: " + hook["command"])
            hook["command"] = shlex.join([sys.argv[3], *words[1:], "--port", sys.argv[2]])
hooks_path.write_text(json.dumps(hooks, indent=2) + "\n")
registry = market / ".agents/plugins/marketplace.json"
registry.parent.mkdir(parents=True, exist_ok=True)
registry.write_text(json.dumps({
    "name": "yard-dogfood",
    "plugins": [{
        "name": "loft",
        "source": {"source": "local", "path": "./plugin"},
        "policy": {"installation": "AVAILABLE", "authentication": "ON_INSTALL"},
        "category": "Developer Tools"
    }]
}, indent=2) + "\n")
print('marketplaces.yard-dogfood={source_type="local",source=' + json.dumps(str(market)) + '}')
PY
)
  CODEX_OPTIONS=(
    --enable plugins --enable hooks
    -c "$CODEX_MARKETPLACE"
    -c 'plugins."loft@yard-dogfood".enabled=true'
    -c 'plugins."loft@ask-loft".enabled=false'
  )
  export PATH="${WRAPPER:h}:$PATH"
  export LOFT_TELEMETRY=off LOFT_UPDATE_CHECK=off
  codex "${CODEX_OPTIONS[@]}" plugin add loft@yard-dogfood --json >/dev/null
  CODEX_CONNECTION=()
  CODEX_AUTO_CONNECTION=true
  CODEX_EXPLICIT_CWD=false
  for argument in "$@"; do
    case "$argument" in
      --remote|--remote=*|--no-daemon) CODEX_AUTO_CONNECTION=false ;;
      --cd|--cd=*|-C|-C?*) CODEX_EXPLICIT_CWD=true ;;
    esac
  done
  if $CODEX_AUTO_CONNECTION; then
    # The version command exits nonzero when no daemon is listening.
    CODEX_DAEMON=$(codex app-server daemon version 2>/dev/null) || CODEX_DAEMON='{}'
    CODEX_SOCKET=$(print -r -- "$CODEX_DAEMON" | python3 -c '
import json, sys
daemon = json.load(sys.stdin)
print(daemon.get("socketPath", "") if daemon.get("status") == "running" else "")
')
    if [[ -n $CODEX_SOCKET ]]; then
      # Config overrides otherwise select an embedded server, even without --no-daemon.
      CODEX_CONNECTION=(--remote "unix://$CODEX_SOCKET")
      if ! $CODEX_EXPLICIT_CWD; then
        CODEX_CONNECTION+=(--cd "$PWD")
      fi
    fi
  fi
  exec codex "${CODEX_CONNECTION[@]}" "${CODEX_OPTIONS[@]}" "$@"
fi

SETTINGS=$(cat <<JSON
{
  "enabledPlugins": { "loft@inline": true },
  "pluginConfigs": {
    "loft": { "options": { "loftCommand": "$WRAPPER", "yardPort": $PORT, "baseBranch": "develop" } },
    "loft@inline": { "options": { "loftCommand": "$WRAPPER", "yardPort": $PORT, "baseBranch": "develop" } }
  }
}
JSON
)

exec claude --plugin-dir "$TREE/plugins/claude/loft" --settings "$SETTINGS" "$@"
