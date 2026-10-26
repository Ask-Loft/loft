#!/bin/zsh
# Start Claude Code or Codex with this worktree's Loft binary and Yard plugin.
# Both start in the caller's working directory. Claude is the default.
#   ./scripts/yard-dogfood.sh            start a new session
#   ./scripts/yard-dogfood.sh --continue resume the last one (Claude flags pass through)
#   ./scripts/yard-dogfood.sh --resume <session-id> resume a specific Claude session
#   ./scripts/yard-dogfood.sh codex      start Codex (requires Python 3)
#   ./scripts/yard-dogfood.sh codex resume <session-id> resume a Codex session
set -euo pipefail

TREE=${0:A:h:h}
PORT=7412
LOFT="$TREE/target/release/loft"
AGENT_COMMAND=claude
if [[ ${1-} == codex || ${1-} == claude ]]; then
  AGENT_COMMAND=$1
  shift
fi

echo "Building loft (release) in $TREE ..."
(
  cd "$TREE"
  cargo build --release --quiet --package loft-cli --target-dir "$TREE/target"
)
LOFT_TELEMETRY=off LOFT_UPDATE_CHECK=off "$LOFT" --version

echo "Yard page: http://127.0.0.1:$PORT/"
if [[ $AGENT_COMMAND == codex ]]; then
  # Generate a local package from the repo source, with this binary and port in its hooks.
  # Codex installs it under a separate identity so it does not replace the released plugin.
  CODEX_MARKETPLACE=$(python3 - "$TREE" "$PORT" <<'PY'
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
            hook["command"] = shlex.join([
                str(tree / "target/release/loft"), *words[1:], "--port", sys.argv[2]
            ])
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
  export PATH="$TREE/target/release:$PATH"
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
    "loft": { "options": { "loftCommand": "$LOFT", "yardPort": $PORT, "baseBranch": "develop" } },
    "loft@inline": { "options": { "loftCommand": "$LOFT", "yardPort": $PORT, "baseBranch": "develop" } }
  }
}
JSON
)

exec claude --plugin-dir "$TREE/plugins/claude/loft" --settings "$SETTINGS" "$@"
