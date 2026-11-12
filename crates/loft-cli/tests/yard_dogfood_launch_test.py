"""Run both real zsh launch paths with isolated agent and restart-helper fixtures."""

import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tempfile
import unittest


WORKSPACE = Path(__file__).resolve().parents[3]
RECORD = '''import json, os, sys
from pathlib import Path
with Path(os.environ["CALLS"]).open("a") as f:
    f.write(json.dumps({"program": Path(sys.argv[0]).name, "args": sys.argv[1:], "cwd": os.getcwd(),
                        "yardHome": os.environ.get("LOFT_YARD_HOME")}) + "\\n")
'''


def read_json(path):
    return json.loads(path.read_text()) if path.exists() else None


class LaunchTests(unittest.TestCase):
    def run_launcher(self, agent, resume=False, failure=False):
        with tempfile.TemporaryDirectory(prefix="yard launch ") as scratch:
            root = Path(scratch).resolve()
            tree = root / "source tree"
            (tree / "scripts").mkdir(parents=True)
            shutil.copyfile(WORKSPACE / "scripts/yard-dogfood.sh", tree / "scripts/yard-dogfood.sh")
            helper = tree / "scripts/yard-dogfood-server.py"
            helper.write_text(RECORD + 'sys.exit(int(os.environ.get("RESTART_FAIL", "0")))\n')
            hooks = tree / "plugins/codex/loft/hooks/hooks.json"
            hooks.parent.mkdir(parents=True)
            hooks.write_text(json.dumps({"hooks": {"SessionStart": [{"hooks": [
                {"command": "loft yard codex hook"},
            ]}]}}))
            binary = tree / "target/release/loft"
            binary.parent.mkdir(parents=True)
            binary.write_text(f"#!{sys.executable}\n" + RECORD)
            binary.chmod(0o755)
            tools = root / "bin"
            tools.mkdir()
            for name in ["cargo", "claude", "codex"]:
                tool = tools / name
                tool.write_text(f"#!{sys.executable}\n" + RECORD + (
                    'if sys.argv[1:4] == ["app-server", "daemon", "version"]:\n'
                    '    print(\'{"status":"stopped"}\')\n'
                ))
                tool.chmod(0o755)
            caller = root / "caller tree"
            caller.mkdir()
            home = root / "home"
            home.mkdir()
            calls = root / "calls.jsonl"
            args = ["zsh", str(tree / "scripts/yard-dogfood.sh"), agent]
            if resume:
                args += ["resume", "session-123"]
            args += ["--model", "example-model"]
            result = subprocess.run(args, cwd=caller, capture_output=True, text=True, env=dict(
                os.environ, PATH=str(tools) + os.pathsep + os.environ["PATH"],
                CALLS=str(calls), RESTART_FAIL="1" if failure else "0", HOME=str(home),
            ), timeout=10)
            records = [json.loads(line) for line in calls.read_text().splitlines()]
            wrapper = tree / ".loft/yard-dogfood/bin/loft"
            ran = None
            if wrapper.exists():
                subprocess.run([str(wrapper), "yard", "protocol"], cwd=caller, timeout=10, env=dict(
                    os.environ, CALLS=str(calls), HOME=str(home)), check=True)
                ran = json.loads(calls.read_text().splitlines()[-1])
            return result, records, str(tree), str(caller), {
                "home": str(home), "wrapper": str(wrapper), "ran": ran,
                "codexHooks": read_json(tree / ".loft/yard-dogfood/codex/plugin/hooks/hooks.json")}

    def test_both_hosts_and_resume_forms_restart_before_agent_start_and_keep_cwd(self):
        for host in ["claude", "codex"]:
            for resume in [False, True]:
                with self.subTest(host=host, resume=resume):
                    result, records, tree, caller, _ = self.run_launcher(host, resume)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(records[0]["program"], "yard-dogfood-server.py")
                    self.assertEqual(records[0]["args"], [tree, "7412"])
                    self.assertEqual(records[0]["cwd"], caller)
                    agent = records[-1]
                    self.assertEqual(agent["program"], host)
                    self.assertEqual(agent["cwd"], caller)
                    suffix = (["--resume" if host == "claude" else "resume", "session-123"]
                              if resume else []) + ["--model", "example-model"]
                    self.assertEqual(agent["args"][-len(suffix):], suffix)

    def test_the_session_s_loft_runs_with_the_dogfood_home(self):
        for host in ["claude", "codex"]:
            with self.subTest(host=host):
                result, records, tree, _, made = self.run_launcher(host)
                self.assertEqual(result.returncode, 0, result.stderr)
                dogfood = str(Path(made["home"]) / ".yard-dogfood")
                ran = made["ran"]
                self.assertIsNotNone(ran, "no wrapper was written")
                self.assertEqual(ran["program"], "loft")
                self.assertEqual(ran["args"], ["yard", "protocol"])
                self.assertEqual(ran["yardHome"], dogfood)
                agent = records[-1]
                self.assertIsNone(agent["yardHome"], "the agent's own environment is left alone")
                if host == "claude":
                    settings = json.loads(agent["args"][agent["args"].index("--settings") + 1])
                    for key in ["loft", "loft@inline"]:
                        self.assertEqual(settings["pluginConfigs"][key]["options"]["loftCommand"],
                                         made["wrapper"])
                else:
                    hooks = made["codexHooks"]
                    command = hooks["hooks"]["SessionStart"][0]["hooks"][0]["command"]
                    words = shlex.split(command)
                    self.assertEqual(words[:4], [made["wrapper"], "yard", "codex", "hook"], command)

    def test_restart_failure_prevents_both_hosts_from_starting(self):
        for host in ["claude", "codex"]:
            with self.subTest(host=host):
                result, records, _, _, _ = self.run_launcher(host, failure=True)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual([record["program"] for record in records], ["yard-dogfood-server.py"])


if __name__ == "__main__":
    unittest.main()
