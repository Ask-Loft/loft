"""Run the launcher's desktop mode through real zsh, with a `claude` that keeps the state its
plugin commands change: the marketplaces, the plugins installed at the local scope, each
project's local settings and the user's settings."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


WORKSPACE = Path(__file__).resolve().parents[3]
RECORD = '''import json, os, sys
from pathlib import Path
with Path(os.environ["CALLS"]).open("a") as f:
    f.write(json.dumps({"program": Path(sys.argv[0]).name, "args": sys.argv[1:], "cwd": os.getcwd()}) + "\\n")
'''
CLAUDE = '''import json, os, sys
from pathlib import Path
args, cwd = sys.argv[1:], Path(os.getcwd())
state_path = Path(os.environ["STATE"])
state = json.loads(state_path.read_text()) if state_path.exists() else {"markets": [], "installed": []}
out = Path(os.environ["OUT"])
with Path(os.environ["CALLS"]).open("a") as f:
    f.write(json.dumps({"program": "claude", "args": args, "cwd": str(cwd),
                        "printedBefore": out.read_text() if out.exists() else ""}) + "\\n")

def settings(path, change):
    held = json.loads(path.read_text()) if path.exists() else {}
    change(held)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(held))

def enable(plugin, value):
    settings(cwd / ".claude/settings.local.json",
             lambda held: held.setdefault("enabledPlugins", {}).__setitem__(plugin, value))

if args[:4] == ["plugin", "marketplace", "list", "--json"]:
    print(json.dumps(state["markets"]))
elif args[:3] == ["plugin", "marketplace", "add"]:
    market = Path(args[3])
    name = json.loads((market / ".claude-plugin/marketplace.json").read_text())["name"]
    state["markets"].append({"name": name, "source": "directory", "path": str(market)})
elif args[:3] == ["plugin", "list", "--json"]:
    print(json.dumps(state["installed"]))
elif args[:2] == ["plugin", "install"] and args[3:] == ["--scope", "local"]:
    state["installed"].append({"id": args[2], "scope": "local", "projectPath": str(cwd), "enabled": True})
    enable(args[2], True)
elif args[:2] in (["plugin", "enable"], ["plugin", "disable"]) and args[3:] == ["--scope", "local"]:
    enable(args[2], args[1] == "enable")
elif args[:2] == ["plugin", "configure"] and args[3:] == ["--values-stdin"]:
    values = json.loads(sys.stdin.read())
    settings(Path(os.environ["HOME"]) / ".claude/settings.json",
             lambda held: held.setdefault("pluginConfigs", {}).setdefault(args[2], {})
             .setdefault("options", {}).update(values))
else:
    sys.exit(f"unexpected claude {args}")
state_path.write_text(json.dumps(state))
'''
MUTATING = {"add", "install", "enable", "disable", "configure"}


def changes(records):
    return [record for record in records if record["program"] == "claude"
            and any(word in MUTATING for word in record["args"][1:3])]


class DesktopTests(unittest.TestCase):
    def setUp(self):
        scratch = tempfile.TemporaryDirectory(prefix="yard desktop ")
        self.addCleanup(scratch.cleanup)
        self.root = Path(scratch.name).resolve()
        self.tree = self.root / "source tree"
        (self.tree / "scripts").mkdir(parents=True)
        for name in ["yard-dogfood.sh", "yard-dogfood-desktop.py"]:
            shutil.copyfile(WORKSPACE / "scripts" / name, self.tree / "scripts" / name)
        (self.tree / "scripts/yard-dogfood-server.py").write_text(RECORD)
        (self.tree / "plugins/claude/loft").mkdir(parents=True)
        tools = self.root / "bin"
        tools.mkdir()
        for name, body in [("claude", CLAUDE), ("cargo", RECORD)]:
            tool = tools / name
            tool.write_text(f"#!{sys.executable}\n" + body)
            tool.chmod(0o755)
        self.home = self.root / "home"
        (self.home / ".claude").mkdir(parents=True)
        self.user_settings = self.home / ".claude/settings.json"
        self.user_settings.write_text(json.dumps({"theme": "dark", "pluginConfigs": {
            "loft@ask-loft": {"options": {"yardPort": 7411}}}}))
        self.project = self.root / "sandbox project"
        self.project.mkdir()
        self.other = self.root / "real work"
        (self.other / ".claude").mkdir(parents=True)
        self.other_settings = '{"enabledPlugins": {"loft@ask-loft": true}}'
        (self.other / ".claude/settings.local.json").write_text(self.other_settings)
        self.calls = self.root / "calls.jsonl"
        self.wrapper = str(self.tree / ".loft/yard-dogfood/bin/loft")
        self.env = dict(os.environ, PATH=str(tools) + os.pathsep + os.environ["PATH"],
                        CALLS=str(self.calls), STATE=str(self.root / "claude-state.json"),
                        HOME=str(self.home))

    def launch(self, *args):
        start = len(self.calls.read_text().splitlines()) if self.calls.exists() else 0
        out = self.root / f"out-{start}.txt"
        with out.open("w") as stdout:
            result = subprocess.run(["zsh", str(self.tree / "scripts/yard-dogfood.sh"), "desktop", *args],
                                    cwd=self.other, stdout=stdout, stderr=subprocess.PIPE, text=True,
                                    env=dict(self.env, OUT=str(out)), timeout=20)
        self.assertEqual(result.returncode, 0, result.stderr + out.read_text())
        records = [json.loads(line) for line in self.calls.read_text().splitlines()][start:]
        return records, out.read_text()

    def local(self, project):
        return json.loads((project / ".claude/settings.local.json").read_text())["enabledPlugins"]

    def test_desktop_switches_only_the_project_to_this_worktree_s_plugin(self):
        records, _ = self.launch(str(self.project))
        self.assertEqual(records[0]["program"], "yard-dogfood-server.py")
        self.assertEqual(records[0]["args"], [str(self.tree), "7412"])
        market = self.tree / ".loft/yard-dogfood/claude"
        manifest = json.loads((market / ".claude-plugin/marketplace.json").read_text())
        self.assertEqual(manifest["name"], "yard-dogfood")
        self.assertEqual(manifest["plugins"][0]["name"], "loft")
        self.assertEqual(manifest["plugins"][0]["source"], "./plugin")
        self.assertTrue((market / "plugin").is_symlink())
        self.assertEqual((market / "plugin").resolve(), self.tree / "plugins/claude/loft")
        made = [(record["args"], record["cwd"]) for record in changes(records)]
        self.assertIn((["plugin", "marketplace", "add", str(market)], str(self.project)), made)
        self.assertIn((["plugin", "install", "loft@yard-dogfood", "--scope", "local"],
                       str(self.project)), made)
        self.assertIn((["plugin", "disable", "loft@ask-loft", "--scope", "local"],
                       str(self.project)), made)
        self.assertEqual(self.local(self.project),
                         {"loft@yard-dogfood": True, "loft@ask-loft": False})
        self.assertTrue(all("--scope" not in args or args[args.index("--scope") + 1] == "local"
                            for args, _ in made if args[1] in {"install", "enable", "disable"}))
        self.assertFalse(any(record["program"] == "claude" and record["args"][0] != "plugin"
                             for record in records), "an agent was started")

    def test_the_settings_of_the_dogfood_plugin_run_this_worktree_s_loft(self):
        self.launch(str(self.project))
        settings = json.loads(self.user_settings.read_text())
        self.assertEqual(settings["pluginConfigs"]["loft@yard-dogfood"]["options"],
                         {"loftCommand": self.wrapper, "yardPort": "7412", "baseBranch": "develop"})
        self.assertEqual(settings["theme"], "dark")
        self.assertEqual(settings["pluginConfigs"]["loft@ask-loft"], {"options": {"yardPort": 7411}})

    def test_each_change_to_the_settings_is_printed_before_it_is_made(self):
        records, _ = self.launch(str(self.project))
        expected = {"add": "Add the marketplace yard-dogfood", "install": "Install loft@yard-dogfood",
                    "disable": "Disable loft@ask-loft", "configure": "Set loft@yard-dogfood's options"}
        made = changes(records)
        self.assertEqual(len(made), 4, made)
        for record in made:
            verb = record["args"][2] if record["args"][1] == "marketplace" else record["args"][1]
            self.assertIn(expected[verb], record["printedBefore"], verb)

    def test_a_second_run_with_the_same_project_changes_nothing(self):
        self.launch(str(self.project))
        before = (self.user_settings.read_text(),
                  (self.project / ".claude/settings.local.json").read_text())
        records, _ = self.launch(str(self.project))
        self.assertEqual(changes(records), [])
        self.assertEqual(before, (self.user_settings.read_text(),
                                  (self.project / ".claude/settings.local.json").read_text()))

    def test_sessions_in_any_other_project_keep_the_installed_plugin(self):
        records, _ = self.launch(str(self.project))
        self.assertTrue(all(record["cwd"] in {str(self.project), str(self.home)}
                            for record in changes(records)))
        self.assertEqual((self.other / ".claude/settings.local.json").read_text(),
                         self.other_settings)

    def test_undo_returns_the_project_to_the_installed_plugin_and_leaves_others_alone(self):
        self.launch(str(self.project))
        records, _ = self.launch("--undo", str(self.project))
        self.assertNotIn("yard-dogfood-server.py", [record["program"] for record in records])
        made = [(record["args"], record["cwd"]) for record in changes(records)]
        self.assertEqual(made, [
            (["plugin", "disable", "loft@yard-dogfood", "--scope", "local"], str(self.project)),
            (["plugin", "enable", "loft@ask-loft", "--scope", "local"], str(self.project)),
        ])
        self.assertEqual(self.local(self.project),
                         {"loft@yard-dogfood": False, "loft@ask-loft": True})
        self.assertEqual((self.other / ".claude/settings.local.json").read_text(),
                         self.other_settings)


if __name__ == "__main__":
    unittest.main()
