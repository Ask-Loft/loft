"""Run both real zsh launch paths with isolated agent and restart-helper fixtures."""

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
            calls = root / "calls.jsonl"
            args = ["zsh", str(tree / "scripts/yard-dogfood.sh"), agent]
            if resume:
                args += ["resume", "session-123"]
            args += ["--model", "example-model"]
            result = subprocess.run(args, cwd=caller, capture_output=True, text=True, env=dict(
                os.environ, PATH=str(tools) + os.pathsep + os.environ["PATH"],
                CALLS=str(calls), RESTART_FAIL="1" if failure else "0",
            ), timeout=10)
            records = [json.loads(line) for line in calls.read_text().splitlines()]
            return result, records, str(tree), str(caller)

    def test_both_hosts_and_resume_forms_restart_before_agent_start_and_keep_cwd(self):
        for host in ["claude", "codex"]:
            for resume in [False, True]:
                with self.subTest(host=host, resume=resume):
                    result, records, tree, caller = self.run_launcher(host, resume)
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

    def test_restart_failure_prevents_both_hosts_from_starting(self):
        for host in ["claude", "codex"]:
            with self.subTest(host=host):
                result, records, _, _ = self.run_launcher(host, failure=True)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual([record["program"] for record in records], ["yard-dogfood-server.py"])


if __name__ == "__main__":
    unittest.main()
