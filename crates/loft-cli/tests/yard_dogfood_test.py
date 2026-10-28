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
        return self.current

    def stop(self, current):
        self.events.append(("stop", current.pid))
        if not self.stuck:
            self.current = None
        return not self.stuck

    def spawn(self, binary, port, cwd):
        pid = 200 + len(self.children)
        child = Child(pid)
        self.children.append(child)
        self.events.append(("spawn", binary, port, cwd))
        if self.rivals:
            self.current = self.rivals.pop(0)
            child.code = 0
        else:
            self.current = server.Server(pid, port, "2.2.1")
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

    def test_same_version_server_is_replaced_before_success(self):
        runtime = Runtime()
        result = self.launch(runtime)
        self.assertEqual(result.pid, 200)
        self.assertEqual(runtime.events, [
            "lock", "build", "version", ("stop", 100),
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
        self.assertFalse(any(isinstance(x, tuple) and x[0] == "stop" for x in runtime.events))

    def test_readiness_is_confirmed_before_a_launch_completes(self):
        runtime = Runtime()
        runtime.ready_after = 3
        clock = Clock()
        self.launch(runtime, clock)
        self.assertGreater(clock.elapsed, 0)
        self.assertEqual(runtime.events.count(("ready", 200)), 4)

    def test_an_unverified_server_times_out_without_starting_another(self):
        runtime = Runtime()
        runtime.stuck = True
        clock = Clock()
        with self.assertRaisesRegex(server.RestartError, "restart.*timed out"):
            self.launch(runtime, clock)
        self.assertEqual(runtime.children, [])
        self.assertGreaterEqual(clock.elapsed, 10)
        self.assertLess(clock.elapsed, 11)

    def test_a_worker_winning_startup_is_replaced_rather_than_accepted(self):
        runtime = Runtime()
        runtime.rivals = [server.Server(300, 7412, "2.2.1")]
        result = self.launch(runtime)
        self.assertEqual(result.pid, 201)
        self.assertIn(("stop", 300), runtime.events)
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
            runtime.current = None
            return child

        runtime.spawn = fail
        with self.assertRaisesRegex(server.RestartError, "exited.*1"):
            self.launch(runtime)


class ProcessSafetyTests(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory()
        self.addCleanup(self.scratch.cleanup)
        self.runtime = server.Runtime(Path(self.scratch.name))
        self.current = server.Server(123, 7412, "2.2.1")
        self.identity = server.Process("/path with spaces/loft", "yard serve --port 7412", "born")

    def test_only_a_verified_server_is_signalled(self):
        with patch.object(self.runtime, "process", return_value=self.identity), \
                patch.object(self.runtime, "owns", return_value=True), \
                patch.object(self.runtime, "running", return_value=self.current), \
                patch.object(server.os, "kill") as kill:
            self.assertTrue(self.runtime.stop(self.current))
        kill.assert_called_once_with(123, server.signal.SIGTERM)

    def test_a_reused_pid_is_never_signalled(self):
        other = server.Process("/bin/something", "yard serve", "later")
        for identities in [[other, other], [self.identity, other]]:
            with self.subTest(identities=identities), \
                    patch.object(self.runtime, "process", side_effect=identities), \
                    patch.object(self.runtime, "owns", return_value=True), \
                    patch.object(self.runtime, "running", return_value=self.current), \
                    patch.object(server.os, "kill") as kill:
                self.assertFalse(self.runtime.stop(self.current))
                kill.assert_not_called()

    def test_a_process_without_the_server_lock_and_listener_is_never_signalled(self):
        with patch.object(self.runtime, "process", return_value=self.identity), \
                patch.object(self.runtime, "owns", return_value=False), \
                patch.object(server.os, "kill") as kill:
            self.assertFalse(self.runtime.stop(self.current))
            kill.assert_not_called()

    def test_a_changed_registration_is_never_signalled(self):
        with patch.object(self.runtime, "process", return_value=self.identity), \
                patch.object(self.runtime, "owns", return_value=True), \
                patch.object(self.runtime, "running", return_value=server.Server(456, 7412, "2.2.1")), \
                patch.object(server.os, "kill") as kill:
            self.assertFalse(self.runtime.stop(self.current))
            kill.assert_not_called()

    def test_stale_metadata_without_a_held_lock_names_no_running_server(self):
        (self.runtime.home / "server.json").write_text('{"pid":123,"port":7412,"version":"2.2.1"}')
        self.assertIsNone(self.runtime.running())

    def test_process_inspection_handles_wide_paths_and_linux_basenames(self):
        for command in ["/path with spaces/loft", "loft"]:
            with self.subTest(command=command), patch.object(self.runtime, "inspect", side_effect=[
                command, "/path with spaces/loft yard serve --port 7412", "born",
            ]):
                self.assertEqual(self.runtime.process(123), self.identity)

    def test_both_the_registered_lock_file_and_listener_must_belong_to_the_pid(self):
        lock = f"p123\nn{self.runtime.home}/server.lock"
        listener = "p123\nn127.0.0.1:7412"
        for files, ports, expected in [
            (lock, listener, True), (lock, "", False), ("", listener, False),
            (lock, "p456\nn127.0.0.1:7412", False),
        ]:
            with self.subTest(files=files, ports=ports), \
                    patch.object(self.runtime, "inspect", side_effect=[files, ports]):
                self.assertEqual(self.runtime.owns(self.current), expected)

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
