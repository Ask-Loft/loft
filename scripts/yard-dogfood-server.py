"""Build and restart the shared development Yard server, before either agent starts."""

from contextlib import contextmanager
from dataclasses import dataclass
import fcntl
import http.client
import json
import os
from pathlib import Path
import signal
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


@dataclass(frozen=True)
class Process:
    executable: str
    arguments: str
    started: str

    def is_server(self):
        return Path(self.executable).name == "loft" and (
            self.arguments == "yard serve" or self.arguments.startswith("yard serve ")
        )


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


def replace(runtime, clock, binary, port, cwd, version):
    until = clock.now() + 10
    child = None
    stopping = set()
    reason = "no replacement has been confirmed"
    succeeded = False
    try:
        while clock.now() < until:
            if child and child.poll() not in (None, 0):
                raise RestartError(f"page server exited with status {child.poll()}")
            try:
                current = runtime.running()
            except RestartError as error:
                reason = str(error)
                pause(clock, until, reason)
                continue
            if current and child and current.pid == child.pid and child.poll() is None:
                if current.port == port and runtime.ready(current, version):
                    succeeded = True
                    return current
                reason = "the replacement has not answered its protocol endpoint"
            elif current:
                if current not in stopping and runtime.stop(current):
                    stopping.add(current)
                reason = "the registered server has not released its lock or cannot be verified"
            elif child is None or child.poll() is not None:
                child = runtime.spawn(binary, port, cwd)
                reason = "the replacement has not registered itself"
            pause(clock, until, f"restart timed out: {reason}")
        raise RestartError(f"restart timed out: {reason}")
    finally:
        if child and not succeeded:
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

    @staticmethod
    def inspect(arguments):
        try:
            result = subprocess.run(arguments, capture_output=True, text=True, timeout=2)
        except (OSError, subprocess.TimeoutExpired) as error:
            raise RestartError(f"cannot inspect the Yard server: {error}") from error
        return result.stdout.strip() if result.returncode == 0 else ""

    def process(self, pid):
        executable = self.inspect(["ps", "-ww", "-p", str(pid), "-o", "comm="])
        full = self.inspect(["ps", "-ww", "-p", str(pid), "-o", "args="])
        started = self.inspect(["ps", "-ww", "-p", str(pid), "-o", "lstart="])
        if not executable or not full or not started:
            return None
        if full.startswith(executable + " "):
            arguments = full[len(executable) + 1:]
        else:
            # Linux's comm is the basename; macOS's wide comm includes the full path.
            prefix, separator, tail = full.partition(" yard serve")
            if not separator or Path(prefix).name != executable:
                return None
            executable, arguments = prefix, "yard serve" + tail
        return Process(executable, arguments, started)

    def owns(self, current):
        pid = str(current.pid)
        lock = self.home / "server.lock"
        files = self.inspect(["lsof", "-nP", "-a", "-p", pid, "-F", "pn", "--", str(lock)])
        listeners = self.inspect([
            "lsof", "-nP", "-a", "-p", pid, f"-iTCP:{current.port}",
            "-sTCP:LISTEN", "-F", "pn",
        ])
        return (f"p{pid}" in files.splitlines() and f"n{lock}" in files.splitlines()
                and f"p{pid}" in listeners.splitlines()
                and f"n127.0.0.1:{current.port}" in listeners.splitlines())

    def stop(self, current):
        before = self.process(current.pid)
        if not before or not before.is_server() or not self.owns(current):
            return False
        # Recheck registration and process lifetime immediately before signalling.
        if self.running() != current or self.process(current.pid) != before:
            return False
        try:
            os.kill(current.pid, signal.SIGTERM)
        except ProcessLookupError:
            return False
        return True

    def spawn(self, binary, port, cwd):
        with self.log.open("ab") as output:
            return subprocess.Popen([
                str(binary), "yard", "serve", "--tree", str(cwd),
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
