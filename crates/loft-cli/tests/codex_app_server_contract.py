"""Isolated real app-server half of the shared accounting port contract."""
import base64
import json
import os
import socket
import struct
import sys


class Client:
    def __init__(self, path):
        self.connection = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.connection.settimeout(10)
        self.connection.connect(str(path))
        key = base64.b64encode(os.urandom(16)).decode()
        self.connection.sendall((f"GET / HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n").encode())
        self.buffer = b""
        while b"\r\n\r\n" not in self.buffer:
            self.buffer += self.connection.recv(4096)
        header, self.buffer = self.buffer.split(b"\r\n\r\n", 1)
        assert b"101 Switching Protocols" in header, header
        self.number = 0
        self.call("initialize", {"clientInfo": {"name": "yard_contract", "version": "1.0"}, "capabilities": {"experimentalApi": True}})
        self.send({"method": "initialized"})

    def exact(self, size):
        while len(self.buffer) < size:
            part = self.connection.recv(max(4096, size - len(self.buffer)))
            if not part:
                raise EOFError("WebSocket closed")
            self.buffer += part
        value, self.buffer = self.buffer[:size], self.buffer[size:]
        return value

    def frame(self, value, opcode=1):
        mask = os.urandom(4)
        size = len(value)
        if size < 126:
            header = bytes([0x80 | opcode, 0x80 | size])
        elif size < 65536:
            header = bytes([0x80 | opcode, 0x80 | 126]) + struct.pack("!H", size)
        else:
            header = bytes([0x80 | opcode, 0x80 | 127]) + struct.pack("!Q", size)
        encoded = bytes(part ^ mask[index % 4] for index, part in enumerate(value))
        self.connection.sendall(header + mask + encoded)

    def send(self, value):
        self.frame(json.dumps(value).encode())

    def receive(self):
        data = b""
        while True:
            first, second = self.exact(2)
            size = second & 127
            if size == 126:
                size = struct.unpack("!H", self.exact(2))[0]
            elif size == 127:
                size = struct.unpack("!Q", self.exact(8))[0]
            mask = self.exact(4) if second & 128 else None
            value = self.exact(size)
            if mask:
                value = bytes(part ^ mask[index % 4] for index, part in enumerate(value))
            opcode = first & 15
            if opcode == 8:
                raise EOFError("WebSocket closed")
            if opcode == 9:
                self.frame(value, 10)
                continue
            if opcode == 10:
                continue
            data += value
            if first & 128:
                return json.loads(data)

    def call(self, method, params):
        self.number += 1
        number = self.number
        self.send({"id": number, "method": method, "params": params})
        while True:
            row = self.receive()
            if row.get("id") == number:
                if "error" in row:
                    raise RuntimeError(row["error"])
                return row["result"]


import shutil
import subprocess
import tempfile
import threading
import time
from pathlib import Path
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


class Provider(BaseHTTPRequestHandler):
    requests = 0

    def log_message(self, *_args):
        pass

    def do_POST(self):
        self.rfile.read(int(self.headers["Content-Length"]))
        Provider.requests += 1
        item = {"id": "message", "type": "message", "role": "assistant",
                "content": [{"type": "output_text", "text": "Fixture complete."}]}
        rows = [
            {"type": "response.output_item.done", "output_index": 0, "item": item},
            {"type": "response.completed", "response": {
                "id": f"fixture_{Provider.requests}", "status": "completed", "output": [item],
                "usage": {"input_tokens": 100, "input_tokens_details": {"cached_tokens": 30, "cache_write_tokens": 7},
                          "output_tokens": 5, "output_tokens_details": {"reasoning_tokens": 2},
                          "total_tokens": 105}}},
        ]
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.end_headers()
        for row in rows:
            self.wfile.write(("data: " + json.dumps(row) + "\n\n").encode())


def run():
    version = subprocess.check_output(["codex", "--version"], text=True).strip()
    assert version == "codex-cli 0.164.0", f"Review the app-server contract pin: {version}"
    # A short root also respects Darwin's Unix socket path limit.
    with tempfile.TemporaryDirectory(prefix="cu-", dir="/tmp") as temporary:
        root = Path(temporary)
        home, work = root / "home", root / "work"
        codex_home = home / ".codex"
        codex_home.mkdir(parents=True)
        work.mkdir()
        endpoint = root / "server.sock"
        provider = ThreadingHTTPServer(("127.0.0.1", 0), Provider)
        threading.Thread(target=provider.serve_forever, daemon=True).start()
        config = '\n'.join([
            'model="gpt-5.4"', 'model_provider="fixture"', 'check_for_update_on_startup=false',
            'approval_policy="never"', '[analytics]', 'enabled=false',
            '[model_providers.fixture]', 'name="Fixture"',
            f'base_url="http://127.0.0.1:{provider.server_port}/v1"',
            'wire_api="responses"', 'requires_openai_auth=false',
            '[projects.' + json.dumps(str(work)) + ']', 'trust_level="trusted"',
        ]) + '\n'
        (codex_home / "config.toml").write_text(config)
        env = {key: value for key, value in os.environ.items()
               if not key.startswith("CODEX_") and not key.endswith("API_KEY")}
        env.update(HOME=str(home), USERPROFILE=str(home), CODEX_HOME=str(codex_home),
                   LOFT_TELEMETRY="off", LOFT_UPDATE_CHECK="off")
        with (root / "errors.log").open("w+") as errors:
            server = subprocess.Popen([shutil.which("codex"), "app-server", "--listen", "unix://" + str(endpoint)],
                                      cwd=work, env=env, stdout=subprocess.DEVNULL, stderr=errors)
            owner = None
            try:
                deadline = time.monotonic() + 10
                while not endpoint.exists() and time.monotonic() < deadline:
                    time.sleep(.02)
                owner = Client(endpoint)
                thread = owner.call("thread/start", {"cwd": str(work), "modelProvider": "fixture",
                                    "model": "gpt-5.4", "approvalPolicy": "never", "sandbox": "read-only"})["thread"]["id"]
                # A native rollout is created lazily by the first turn. Seed historical
                # usage before the observer attaches, then verify it is never imported.
                owner.call("turn/start", {"threadId": thread, "input": [{"type": "text", "text": "Seed local fixture history."}]})
                while owner.receive().get("method") != "turn/completed":
                    pass
                print(json.dumps({"socket": str(endpoint), "thread": thread}), flush=True)
                for _line in sys.stdin:
                    owner.call("turn/start", {"threadId": thread, "input": [{"type": "text", "text": "Complete the local fixture."}]})
                    while owner.receive().get("method") != "turn/completed":
                        pass
                assert Provider.requests == 3, Provider.requests
            finally:
                if owner:
                    owner.connection.close()
                server.terminate()
                try:
                    server.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    server.kill()
                    server.wait()
                provider.shutdown()


if __name__ == "__main__":
    run()
