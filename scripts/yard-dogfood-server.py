"""Build loft and have it replace the shared development Yard server, before either agent starts."""

from contextlib import contextmanager
from dataclasses import dataclass
import fcntl
import http.client
import json
import os
from pathlib import Path
import subprocess
import sys
import time


class RestartError(Exception):
    pass


@dataclass(frozen=True)
class Server:
    pid: int
    port: int
    version: str


class Clock:
    def now(self):
        return time.monotonic()

    def wait(self, seconds):
        time.sleep(seconds)


def pause(clock, until, reason):
    remaining = until - clock.now()
    if remaining <= 0:
        raise RestartError(reason)
    clock.wait(min(0.05, remaining))


# `loft yard serve --replace` waits ten seconds for the running server to stop.
WAITING = 15


def replace(runtime, clock, binary, port, cwd, version):
    """Starts `loft yard serve --replace`, which asks the running page server to stop whatever
    its version and serves in its place, and waits until it serves and answers."""
    until = clock.now() + WAITING
    child = runtime.spawn(binary, port, cwd)
    reason = "the replacement has not registered itself"
    succeeded = False
    try:
        while True:
            if child.poll() is not None:
                raise RestartError(f"page server exited with status {child.poll()}")
            try:
                current = runtime.running()
            except RestartError as error:
                current, reason = None, str(error)
            if current and current.pid == child.pid:
                if current.port == port and runtime.ready(current, version):
                    succeeded = True
                    return current
                reason = "the replacement has not answered its protocol endpoint"
            pause(clock, until, f"restart timed out: {reason}")
    finally:
        if not succeeded:
            runtime.cleanup(child)


def start(runtime, clock, tree, port, cwd):
    with runtime.exclusive(clock):
        runtime.build(tree)
        binary = tree / "target/release/loft"
        version = runtime.version(binary)
        return replace(runtime, clock, binary, port, cwd, version)


class Runtime:
    def __init__(self, home=None):
        self.home = (home if home is not None else Path.home() / ".yard").resolve()
        self.home.mkdir(parents=True, exist_ok=True)
        self.log = self.home / "dogfood-server.log"
        self.env = dict(os.environ, LOFT_TELEMETRY="off", LOFT_UPDATE_CHECK="off")

    @contextmanager
    def exclusive(self, clock):
        # Never remove a lock file: unlinking it could create two independent locks.
        with (self.home / "dogfood.lock").open("a+b") as handle:
            until = clock.now() + 900
            while True:
                try:
                    fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
                    break
                except BlockingIOError:
                    pause(clock, until, "timed out waiting for another dogfood build/restart")
            try:
                yield
            finally:
                fcntl.flock(handle, fcntl.LOCK_UN)

    def build(self, tree):
        print(f"Building loft (release) in {tree} ...", flush=True)
        result = subprocess.run([
            "cargo", "build", "--release", "--quiet", "--package", "loft-cli",
            "--target-dir", str(tree / "target"),
        ], cwd=tree, env=self.env)
        if result.returncode:
            raise RestartError(f"build failed with status {result.returncode}; existing server kept")

    def version(self, binary):
        result = subprocess.run([str(binary), "--version", "--format", "json"],
                                env=self.env, capture_output=True, text=True, check=True)
        version = json.loads(result.stdout)["version"]
        print(f"loft {version}", flush=True)
        return version

    def held(self):
        try:
            with (self.home / "server.lock").open("rb") as handle:
                try:
                    fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
                except BlockingIOError:
                    return True
                fcntl.flock(handle, fcntl.LOCK_UN)
        except FileNotFoundError:
            pass
        return False

    def running(self):
        if not self.held():
            return None
        try:
            named = json.loads((self.home / "server.json").read_text())
            pid, port, version = named["pid"], named["port"], named["version"]
            if type(pid) is not int or pid <= 1 or type(port) is not int or not 0 < port < 65536:
                raise ValueError("invalid server process or port")
            if not isinstance(version, str):
                raise ValueError("invalid server version")
            return Server(pid, port, version)
        except (OSError, ValueError, KeyError, TypeError) as error:
            raise RestartError(f"held server lock has no valid registration: {error}") from error

    def spawn(self, binary, port, cwd):
        with self.log.open("ab") as output:
            return subprocess.Popen([
                str(binary), "yard", "serve", "--replace", "--tree", str(cwd),
                "--port", str(port), "--format", "json",
            ], cwd=cwd, env=self.env, stdin=subprocess.DEVNULL,
                stdout=output, stderr=output, start_new_session=True, close_fds=True)

    @staticmethod
    def ready(current, version):
        connection = http.client.HTTPConnection("127.0.0.1", current.port, timeout=0.3)
        try:
            connection.request("GET", "/api/version")
            response = connection.getresponse()
            return response.status == 200 and json.loads(response.read(65536))["loft"] == version
        except (OSError, ValueError, KeyError, http.client.HTTPException):
            return False
        finally:
            connection.close()

    @staticmethod
    def cleanup(child):
        if child.poll() is None:
            child.terminate()
            try:
                child.wait(timeout=1)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait(timeout=1)


def main():
    runtime = Runtime()
    try:
        tree = Path(sys.argv[1]).resolve()
        port = int(sys.argv[2])
        running = start(runtime, Clock(), tree, port, Path.cwd())
        print(f"Yard restarted: PID {running.pid}, http://127.0.0.1:{running.port}/", flush=True)
        return 0
    except (RestartError, OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"yard-dogfood: {error}\nServer log: {runtime.log}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
