"""Switch one project's Claude Code sessions, Claude Desktop's among them, to this worktree's Loft
plugin and loft, or back to the installed ones.

  yard-dogfood-desktop.py <tree> <port> <loft command> <project>          switch the project
  yard-dogfood-desktop.py <tree> <port> <loft command> <project> --undo   switch it back

The worktree's plugin is installed as `loft@yard-dogfood` from a local marketplace whose plugin is
a link to `plugins/claude/loft`, so Claude Code reads it from the worktree and `/reload-plugins`
takes up an edit. Both plugins are switched at the local scope, which only the project reads, so
every other project keeps the installed `loft@ask-loft`. Each change to the person's Claude Code
settings is printed before it is made, and a change already in place is not made again.
"""
import json
import os
from pathlib import Path
import subprocess
import sys

MARKETPLACE = "yard-dogfood"
DEV = f"loft@{MARKETPLACE}"
INSTALLED = "loft@ask-loft"


class DesktopError(Exception):
    pass


def claude(args, cwd, stdin=None):
    """Runs `claude <args>` in `cwd` and returns its standard output."""
    result = subprocess.run(["claude", *args], cwd=cwd, input=stdin, capture_output=True,
                            text=True)
    if result.returncode:
        raise DesktopError(f"claude {' '.join(args)} failed: {result.stderr.strip()}")
    return result.stdout


def say(text):
    print(text, flush=True)


def read_json(path, empty):
    try:
        return json.loads(path.read_text())
    except FileNotFoundError:
        return empty


def keep_marketplace(tree):
    """Writes the local marketplace beside the worktree's build and returns its directory."""
    market = tree / ".loft/yard-dogfood/claude"
    manifest = market / ".claude-plugin/marketplace.json"
    manifest.parent.mkdir(parents=True, exist_ok=True)
    manifest.write_text(json.dumps({
        "name": MARKETPLACE,
        "owner": {"name": "Loft dogfood"},
        "plugins": [{
            "name": "loft",
            "source": "./plugin",
            "description": "This worktree's Loft plugin, read from plugins/claude/loft.",
        }],
    }, indent=2) + "\n")
    link = market / "plugin"
    target = tree / "plugins/claude/loft"
    if link.is_symlink() and Path(os.readlink(link)) == target:
        return market
    if link.is_symlink() or link.exists():
        link.unlink()
    link.symlink_to(target, target_is_directory=True)
    return market


def local_enabled(project):
    """The plugins the project's local settings enable or disable, by id."""
    settings = read_json(project / ".claude/settings.local.json", {})
    return settings.get("enabledPlugins", {})


def installed_here(project):
    """Whether `loft@yard-dogfood` is installed at the local scope of `project`."""
    listed = json.loads(claude(["plugin", "list", "--json"], project) or "[]")
    return any(entry.get("id") == DEV and entry.get("scope") == "local"
               and Path(entry.get("projectPath", "")) == project for entry in listed)


def switch(project, plugin, enable):
    if local_enabled(project).get(plugin) is enable:
        return
    verb = "enable" if enable else "disable"
    say(f"{verb.capitalize()} {plugin} in {project}'s local settings (.claude/settings.local.json)")
    claude(["plugin", verb, plugin, "--scope", "local"], project)


def configure(home, options):
    """Sets the options of `loft@yard-dogfood` in the person's user settings, where they differ."""
    current = read_json(home / ".claude/settings.json", {})
    stored = current.get("pluginConfigs", {}).get(DEV, {}).get("options", {})
    if all(str(stored.get(key)) == value for key, value in options.items()):
        return
    say(f"Set {DEV}'s options in ~/.claude/settings.json: {json.dumps(options)}")
    claude(["plugin", "configure", DEV, "--values-stdin"], home, stdin=json.dumps(options))


def desktop(tree, port, command, project, home):
    market = keep_marketplace(tree)
    listed = json.loads(claude(["plugin", "marketplace", "list", "--json"], project) or "[]")
    known = next((entry for entry in listed if entry.get("name") == MARKETPLACE), None)
    if known is None:
        say(f"Add the marketplace {MARKETPLACE} from {market} to your user settings")
        claude(["plugin", "marketplace", "add", str(market)], project)
    elif Path(known.get("path", known.get("installLocation", market))) != market:
        raise DesktopError(f"the marketplace {MARKETPLACE} is already added from "
                           f"{known.get('path') or known.get('installLocation')}; remove it with "
                           f"`claude plugin marketplace remove {MARKETPLACE}` first")
    if not installed_here(project):
        say(f"Install {DEV} in {project}'s local settings")
        claude(["plugin", "install", DEV, "--scope", "local"], project)
    switch(project, DEV, True)
    switch(project, INSTALLED, False)
    configure(home, {"loftCommand": command, "yardPort": str(port), "baseBranch": "develop"})
    say(f"{project} now runs this worktree's Loft plugin; Yard page: http://127.0.0.1:{port}/")
    say("Start a new session there, or run /reload-plugins in an open one.")


def undo(project):
    switch(project, DEV, False)
    switch(project, INSTALLED, True)
    say(f"{project} runs the installed Loft plugin again; start a new session there.")


def main(argv):
    try:
        tree, port, command, project = Path(argv[0]).resolve(), int(argv[1]), argv[2], argv[3]
        project = Path(project).resolve()
        if not project.is_dir():
            raise DesktopError(f"no project directory {project}")
        if argv[4:] == ["--undo"]:
            undo(project)
        elif argv[4:] == []:
            desktop(tree, port, command, project, Path.home())
        else:
            raise DesktopError(f"unexpected arguments {argv[4:]}")
        return 0
    except (DesktopError, OSError, ValueError, IndexError) as error:
        print(f"yard-dogfood desktop: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
