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


class Launch:
    """One run of the launcher in a scratch tree, kept until the test ends."""

    def __init__(self, test, agent, resume=False, failure=False):
        scratch = tempfile.TemporaryDirectory(prefix="yard launch ")
        test.addCleanup(scratch.cleanup)
        root = Path(scratch.name).resolve()
        self.tree = root / "source tree"
        (self.tree / "scripts").mkdir(parents=True)
        shutil.copyfile(WORKSPACE / "scripts/yard-dogfood.sh", self.tree / "scripts/yard-dogfood.sh")
        helper = self.tree / "scripts/yard-dogfood-server.py"
        helper.write_text(RECORD + 'sys.exit(int(os.environ.get("RESTART_FAIL", "0")))\n')
        hooks = self.tree / "plugins/codex/loft/hooks/hooks.json"
        hooks.parent.mkdir(parents=True)
        hooks.write_text(json.dumps({"hooks": {"SessionStart": [{"hooks": [
            {"command": "loft yard codex hook"},
        ]}]}}))
        binary = self.tree / "target/release/loft"
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
        self.caller = root / "caller tree"
        self.caller.mkdir()
        self.home = root / "home"
        self.home.mkdir()
        self.calls = root / "calls.jsonl"
        self.wrapper = self.tree / ".loft/yard-dogfood/bin/loft"
        args = ["zsh", str(self.tree / "scripts/yard-dogfood.sh"), agent]
        if resume:
            args += ["resume", "session-123"]
        args += ["--model", "example-model"]
        self.result = subprocess.run(args, cwd=self.caller, capture_output=True, text=True, env=dict(
            os.environ, PATH=str(tools) + os.pathsep + os.environ["PATH"],
            CALLS=str(self.calls), RESTART_FAIL="1" if failure else "0", HOME=str(self.home),
        ), timeout=10)
        self.records = self.recorded()

    def recorded(self):
        return [json.loads(line) for line in self.calls.read_text().splitlines()]

    def run_wrapper(self, *args):
        """Runs the wrapper the launcher wrote, and gives what the `loft` it ran was given."""
        subprocess.run([str(self.wrapper), *args], cwd=self.caller, timeout=10, check=True,
                       env=dict(os.environ, CALLS=str(self.calls), HOME=str(self.home)))
        return self.recorded()[-1]


class LaunchTests(unittest.TestCase):
    def test_both_hosts_and_resume_forms_restart_before_agent_start_and_keep_cwd(self):
        for host in ["claude", "codex"]:
            for resume in [False, True]:
                with self.subTest(host=host, resume=resume):
                    launch = Launch(self, host, resume)
                    self.assertEqual(launch.result.returncode, 0, launch.result.stderr)
                    records = launch.records
                    self.assertEqual(records[0]["program"], "yard-dogfood-server.py")
                    self.assertEqual(records[0]["args"], [
                        str(launch.tree), "7412", str(launch.home / ".yard-dogfood")])
                    self.assertEqual(records[0]["cwd"], str(launch.caller))
                    agent = records[-1]
                    self.assertEqual(agent["program"], host)
                    self.assertEqual(agent["cwd"], str(launch.caller))
                    suffix = (["--resume" if host == "claude" else "resume", "session-123"]
                              if resume else []) + ["--model", "example-model"]
                    self.assertEqual(agent["args"][-len(suffix):], suffix)

    def test_the_session_s_loft_runs_with_the_dogfood_home(self):
        for host in ["claude", "codex"]:
            with self.subTest(host=host):
                launch = Launch(self, host)
                self.assertEqual(launch.result.returncode, 0, launch.result.stderr)
                agent = launch.records[-1]
                # Cargo starts every test with an empty `LOFT_YARD_HOME`, which loft ignores.
                self.assertFalse(agent["yardHome"], "the agent's own environment is left alone")
                self.assertEqual([path.name for path in launch.wrapper.parent.iterdir()], ["loft"])
                ran = launch.run_wrapper("yard", "protocol")
                self.assertEqual(ran["program"], "loft")
                self.assertEqual(ran["args"], ["yard", "protocol"])
                self.assertEqual(ran["yardHome"], str(launch.home / ".yard-dogfood"))
                wrapper = str(launch.wrapper)
                if host == "claude":
                    settings = json.loads(agent["args"][agent["args"].index("--settings") + 1])
                    for key in ["loft", "loft@inline"]:
                        self.assertEqual(settings["pluginConfigs"][key]["options"]["loftCommand"],
                                         wrapper)
                else:
                    package = launch.tree / ".loft/yard-dogfood/codex/plugin/hooks/hooks.json"
                    hooks = json.loads(package.read_text())
                    command = hooks["hooks"]["SessionStart"][0]["hooks"][0]["command"]
                    self.assertEqual(shlex.split(command)[:4], [wrapper, "yard", "codex", "hook"],
                                     command)

    def test_restart_failure_prevents_both_hosts_from_starting(self):
        for host in ["claude", "codex"]:
            with self.subTest(host=host):
                launch = Launch(self, host, failure=True)
                self.assertNotEqual(launch.result.returncode, 0)
                self.assertEqual([record["program"] for record in launch.records],
                                 ["yard-dogfood-server.py"])


if __name__ == "__main__":
    unittest.main()
