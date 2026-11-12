"""Local contract: actual Loft and its locks, under an isolated home and git simulator."""

import fcntl
import hashlib
import http.client
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time
import unittest
from urllib.parse import urlencode

from yard_dogfood_test import server


SETUP = json.loads(Path(sys.argv[1]).read_text())
sys.argv = sys.argv[:1]
WORKSPACE = Path(__file__).resolve().parents[3]


def free_port():
    with socket.socket() as port:
        port.bind(("127.0.0.1", 0))
        return port.getsockname()[1]


def without_yard_home(env):
    """`env` with no `LOFT_YARD_HOME`: the installed loft's, and the launcher's own, so that the
    helper is seen to name the dogfood home to the `loft` it starts by itself."""
    return {key: value for key, value in env.items() if key != "LOFT_YARD_HOME"}


class ContractTests(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory(prefix="yard server contract ")
        self.addCleanup(self.scratch.cleanup)
        self.root = Path(self.scratch.name).resolve()
        self.home = self.root / "home"
        self.home.mkdir()
        self.tree = Path(SETUP["cwd"]).resolve()
        self.source = self.root / "source tree"
        self.binary = self.source / "target/release/loft"
        self.binary.parent.mkdir(parents=True)
        shutil.copyfile(SETUP["program"], self.binary)
        self.binary.chmod(0o755)
        tools = self.root / "bin"
        tools.mkdir()
        cargo = tools / "cargo"
        cargo.write_text(f"#!{sys.executable}\nimport os,sys\nsys.exit(int(os.environ.get('BUILD_FAIL','0')))\n")
        cargo.chmod(0o755)
        # Keep real git off PATH.
        self.env = os.environ.copy()
        for key, value in SETUP["env"].items():
            if value is None:
                self.env.pop(key, None)
            else:
                self.env[key] = value
        self.env.update(HOME=str(self.home), PATH=str(tools) + os.pathsep + self.env["PATH"],
                        LOFT_TELEMETRY="off", LOFT_UPDATE_CHECK="off",
                        LOFT_YARD_HOME=str(self.home / ".yard-dogfood"))
        self.runtime = server.Runtime(self.home / ".yard-dogfood")
        self.runtime.env = self.env
        self.port = free_port()
        self.children = []
        self.addCleanup(self.stop_servers)

    def stop_servers(self):
        try:
            try:
                current = self.runtime.running()
            except server.RestartError:
                current = None
            # The helper's server outlives it in its own session; the isolated home names it.
            if current and current.pid != os.getpid():
                os.kill(current.pid, signal.SIGTERM)
                self.within(lambda: not self.runtime.held())
        finally:
            for child in self.children:
                if child.poll() is None:
                    child.terminate()
                child.wait(timeout=5)

    @staticmethod
    def within(check):
        # A bounded process/socket readiness assertion, with no fixed sleep or policy delay.
        until = time.monotonic() + 5
        while time.monotonic() < until:
            try:
                if check():
                    return
            except server.RestartError:
                pass
        raise AssertionError("local process/socket did not become ready")

    def initial_server(self, runtime=None, port=None):
        runtime = runtime or self.runtime
        child = runtime.spawn(self.binary, port or self.port, self.tree)
        self.children.append(child)

        def ready():
            current = runtime.running()
            return current and current.pid == child.pid and runtime.ready(current, current.version)

        self.within(ready)
        return child

    def helper(self, **env):
        child = subprocess.Popen([
            sys.executable, str(WORKSPACE / "scripts/yard-dogfood-server.py"),
            str(self.source), str(self.port), str(self.runtime.home),
        ], cwd=self.tree, env=without_yard_home(dict(self.env, **env)), stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, text=True)
        self.children.append(child)
        return child

    def finished(self, child, expected=0):
        out, err = child.communicate(timeout=20)
        self.assertEqual(child.returncode, expected, out + err + self.runtime.log.read_text())
        return out

    def get(self, client, path):
        client.request("GET", path)
        reply = client.getresponse()
        self.assertEqual(reply.status, 200)
        return json.loads(reply.read())

    def test_same_version_restart_preserves_trees_tokens_and_a_clients_recorded_log(self):
        original = self.initial_server()
        current = self.runtime.running()
        other = self.tree / "other-tree"
        other.mkdir(exist_ok=True)
        subprocess.run([str(self.binary), "yard", "serve", "--tree", str(other), "--format", "json"],
                       cwd=self.tree, env=self.env, capture_output=True, check=True)
        token = (self.runtime.home / "token").read_bytes()
        trees = (self.runtime.home / "trees.json").read_bytes()
        files = {str(p): hashlib.sha256(p.read_bytes()).hexdigest()
                 for p in (self.tree / ".yard").rglob("*") if p.is_file()}
        client = http.client.HTTPConnection("127.0.0.1", self.port, timeout=2)
        self.addCleanup(client.close)
        path = "/api/log?" + urlencode({"tree": str(self.tree), "branch": "feat/shelf"})
        before = self.get(client, path)
        out = self.finished(self.helper())
        original.wait(timeout=5)
        replacement = self.runtime.running()
        self.assertNotEqual(replacement.pid, current.pid)
        self.assertEqual(replacement.version, current.version)
        self.assertEqual(replacement.port, self.port)
        self.assertIn(f"PID {replacement.pid}", out)
        after = self.get(client, path)
        self.assertEqual(after["yardLog"], before["yardLog"])
        self.assertEqual((self.runtime.home / "token").read_bytes(), token)
        self.assertEqual((self.runtime.home / "trees.json").read_bytes(), trees)
        self.assertEqual(files, {str(p): hashlib.sha256(p.read_bytes()).hexdigest()
                               for p in (self.tree / ".yard").rglob("*") if p.is_file()})

    def test_concurrent_launchers_finish_with_one_verified_server(self):
        original = self.initial_server()
        first, second = self.helper(), self.helper()
        reports = self.finished(first) + self.finished(second)
        original.wait(timeout=5)
        current = self.runtime.running()
        self.assertIn(f"PID {current.pid}", reports)
        self.assertTrue(self.runtime.ready(current, current.version))
        with (self.runtime.home / "server.lock").open("rb") as handle:
            with self.assertRaises(BlockingIOError):
                fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
        starts = [json.loads(line) for line in self.runtime.log.read_text().splitlines()]
        self.assertEqual(sum(line.get("outcome") == "serving" for line in starts), 3)

    def test_the_page_server_kept_in_the_default_home_keeps_running_through_a_restart(self):
        installed = server.Runtime(self.home / ".yard")
        installed.env = without_yard_home(self.env)
        stable = self.initial_server(installed, free_port())
        before = installed.running()
        self.initial_server()
        self.finished(self.helper())
        self.assertIsNone(stable.poll())
        self.assertEqual(installed.running(), before)
        self.assertTrue(installed.ready(before, before.version))
        self.assertFalse((self.home / ".yard/stop.json").exists())
        current = self.runtime.running()
        self.assertEqual(current.port, self.port)
        self.assertNotEqual(current.pid, before.pid)

    def test_a_failed_build_leaves_the_real_server_answering(self):
        original = self.initial_server()
        out = self.finished(self.helper(BUILD_FAIL="1"), expected=1)
        self.assertNotIn("Yard restarted", out)
        self.assertIsNone(original.poll())
        current = self.runtime.running()
        self.assertEqual(current.pid, original.pid)
        self.assertTrue(self.runtime.ready(current, current.version))


    def test_a_real_unrelated_process_named_by_stale_metadata_is_left_running(self):
        # This process holds the lock as a page server built before `--replace` would: it never
        # reads the stop request. The passing clock ends the replacement's wait at once.
        with (self.runtime.home / "server.lock").open("a+b") as handle:
            fcntl.flock(handle, fcntl.LOCK_EX)
            named = server.Server(os.getpid(), self.port, "2.2.1")
            (self.runtime.home / "server.json").write_text(json.dumps(vars(named)))
            self.assertEqual(self.runtime.running(), named)
            child = self.helper(LOFT_CLOCK="passing")
            out, err = child.communicate(timeout=20)
            self.assertEqual(child.returncode, 1, out + err)
            self.assertNotIn("Yard restarted", out)
            self.assertIn(f"process {os.getpid()}", self.runtime.log.read_text())
            self.assertEqual(self.runtime.running(), named)
            os.kill(os.getpid(), 0)


if __name__ == "__main__":
    unittest.main()
