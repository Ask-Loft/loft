"""Focused dogfood restart policies; all waits use a simulated clock."""

from contextlib import contextmanager
import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch


SOURCE = Path(__file__).resolve().parents[3] / "scripts/yard-dogfood-server.py"
SPEC = importlib.util.spec_from_file_location("dogfood_server", SOURCE)
server = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = server
SPEC.loader.exec_module(server)


class Clock:
    def __init__(self):
        self.elapsed = 0

    def now(self):
        return self.elapsed

    def wait(self, seconds):
        self.elapsed += seconds


class Child:
    def __init__(self, pid, code=None):
        self.pid = pid
        self.code = code

    def poll(self):
        return self.code


class Runtime:
    def __init__(self):
        self.events = []
        self.current = server.Server(100, 7412, "2.2.1")
        self.children = []
        self.rivals = []
        self.stuck = False
        self.failure = None
        self.ready_after = 0

    @contextmanager
    def exclusive(self, clock):
        self.events.append("lock")
        yield
        self.events.append("unlock")

    def build(self, tree):
        self.events.append("build")
        if self.failure:
            raise server.RestartError(self.failure)

    def version(self, binary):
        self.events.append("version")
        return "2.2.1"

    def running(self):
        # A rival that holds the lock first is replaced in turn by the child that replaces it.
        if self.rivals:
            self.current = self.rivals.pop(0)
        elif self.children and not self.stuck:
            child = self.children[-1]
            self.current = server.Server(child.pid, 7412, "2.2.1")
        return self.current

    def spawn(self, binary, port, cwd):
        pid = 200 + len(self.children)
        child = Child(pid)
        self.children.append(child)
        self.events.append(("spawn", binary, port, cwd))
        return child

    def ready(self, current, version):
        self.events.append(("ready", current.pid))
        self.ready_after -= 1
        return self.ready_after < 0

    def cleanup(self, child):
        self.events.append(("cleanup", child.pid))


class RestartTests(unittest.TestCase):
    def launch(self, runtime, clock=None):
        return server.start(runtime, clock or Clock(), Path("/source"), 7412, Path("/caller"))

    def test_the_running_server_is_replaced_by_one_started_to_replace_it(self):
        runtime = Runtime()
        result = self.launch(runtime)
        self.assertEqual(result.pid, 200)
        self.assertEqual(runtime.events, [
            "lock", "build", "version",
            ("spawn", Path("/source/target/release/loft"), 7412, Path("/caller")),
            ("ready", 200), "unlock",
        ])

    def test_build_failure_leaves_the_existing_server_alone(self):
        runtime = Runtime()
        runtime.failure = "build failed"
        with self.assertRaisesRegex(server.RestartError, "build failed"):
            self.launch(runtime)
        self.assertEqual(runtime.current.pid, 100)
        self.assertEqual(runtime.children, [])

    def test_readiness_is_confirmed_before_a_launch_completes(self):
        runtime = Runtime()
        runtime.ready_after = 3
        clock = Clock()
        self.launch(runtime, clock)
        self.assertGreater(clock.elapsed, 0)
        self.assertEqual(runtime.events.count(("ready", 200)), 4)

    def test_a_server_that_is_not_replaced_times_out_and_the_child_is_cleaned_up(self):
        runtime = Runtime()
        runtime.stuck = True
        clock = Clock()
        with self.assertRaisesRegex(server.RestartError, "restart timed out"):
            self.launch(runtime, clock)
        self.assertEqual(len(runtime.children), 1)
        self.assertIn(("cleanup", 200), runtime.events)
        self.assertGreaterEqual(clock.elapsed, server.WAITING)
        self.assertLess(clock.elapsed, server.WAITING + 1)

    def test_a_rival_that_registers_first_is_not_accepted_as_the_replacement(self):
        runtime = Runtime()
        runtime.rivals = [server.Server(300, 7412, "2.2.1")]
        result = self.launch(runtime)
        self.assertEqual(result.pid, 200)
        self.assertNotIn(("ready", 300), runtime.events)

    def test_an_unready_child_is_cleaned_up_on_a_bounded_failure(self):
        runtime = Runtime()
        runtime.ready_after = 10000
        with self.assertRaisesRegex(server.RestartError, "timed out"):
            self.launch(runtime)
        self.assertIn(("cleanup", 200), runtime.events)

    def test_an_exited_child_is_reported_as_a_startup_failure(self):
        runtime = Runtime()
        original = runtime.spawn

        def fail(*args):
            child = original(*args)
            child.code = 1
            return child

        runtime.spawn = fail
        with self.assertRaisesRegex(server.RestartError, "exited.*1"):
            self.launch(runtime)


class RuntimeTests(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory()
        self.addCleanup(self.scratch.cleanup)
        self.runtime = server.Runtime(Path(self.scratch.name))

    def test_the_replacement_is_started_with_replace(self):
        with patch.object(server.subprocess, "Popen") as popen:
            self.runtime.spawn(Path("/source/loft"), 7412, Path("/caller"))
        arguments = popen.call_args.args[0]
        self.assertEqual(arguments[:4], ["/source/loft", "yard", "serve", "--replace"])
        self.assertIn("--tree", arguments)
        self.assertEqual(arguments[arguments.index("--port") + 1], "7412")

    def test_stale_metadata_without_a_held_lock_names_no_running_server(self):
        (self.runtime.home / "server.json").write_text('{"pid":123,"port":7412,"version":"2.2.1"}')
        self.assertIsNone(self.runtime.running())

    def test_build_lock_contention_uses_a_simulated_clock_and_releases_on_error(self):
        clock = Clock()
        with patch.object(server.fcntl, "flock", side_effect=[BlockingIOError(), None, None]) as flock:
            with self.assertRaisesRegex(ValueError, "build failed"), self.runtime.exclusive(clock):
                raise ValueError("build failed")
            self.assertGreater(clock.elapsed, 0)
            self.assertEqual(flock.call_args.args[1], server.fcntl.LOCK_UN)

    def test_build_lock_timeout_does_not_start_a_build(self):
        clock = Clock()
        with patch.object(server.fcntl, "flock", side_effect=BlockingIOError()), \
                patch.object(self.runtime, "build") as build:
            with self.assertRaisesRegex(server.RestartError, "another dogfood"), \
                    self.runtime.exclusive(clock):
                self.runtime.build(Path("/source"))
            build.assert_not_called()
        self.assertEqual(clock.elapsed, 900)


if __name__ == "__main__":
    unittest.main()
