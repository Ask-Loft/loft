"""On-demand real Codex contract; every model response comes from a loopback fixture."""

import json
import os
from pathlib import Path
import queue
import socket
import subprocess
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


def free_port():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


class Provider(BaseHTTPRequestHandler):
    requests = 0
    home = None

    def log_message(self, *_args):
        pass

    def do_POST(self):
        self.rfile.read(int(self.headers["Content-Length"]))
        Provider.requests += 1
        if Provider.requests == 1:
            deadline = time.monotonic() + 5
            while not (Provider.home / ".yard/server.json").exists() and time.monotonic() < deadline:
                time.sleep(0.02)
            assert (Provider.home / ".yard/server.json").exists(), "The native worker did not start the page server"
        if Provider.requests == 1:
            item = {
                "id": "fc_fixture", "type": "function_call", "call_id": "call_fixture",
                "name": "exec_command",
                "arguments": json.dumps({"cmd": "printf fixture", "yield_time_ms": 1000}),
            }
        else:
            item = {"id": "msg_fixture", "type": "message", "role": "assistant",
                    "content": [{"type": "output_text", "text": "Fixture complete."}]}
        rows = [
            {"type": "response.output_item.done", "output_index": 0, "item": item},
            {"type": "response.completed", "response": {
                "id": f"resp_fixture_{Provider.requests}", "status": "completed", "output": [item],
                "usage": {"input_tokens": 10, "output_tokens": 3, "total_tokens": 13},
            }},
        ]
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.end_headers()
        for row in rows:
            self.wfile.write(("data: " + json.dumps(row) + "\n\n").encode())


def run(loft, work, home):
    version = subprocess.check_output(["codex", "--version"], text=True)
    assert version.strip() == "codex-cli 0.164.0", f"Review the Codex contract pin: {version}"
    codex_home = home / "codex-contract"
    codex_home.mkdir()
    (codex_home / "config.toml").write_text("[features]\nhooks = true\n")
    port = free_port()
    # Only this isolated fixture bypasses trust review; installation never does.
    normal = f'"{loft}" yard codex hook --port {port}'
    watch = f'"{loft}" yard codex watch --port {port}'
    names = ["SessionStart", "UserPromptSubmit", "PreToolUse", "PostToolUse", "Stop", "SessionEnd"]
    hooks = {name: [{"hooks": [{"type": "command", "command": normal}]}] for name in names}
    hooks["SessionStart"][0]["hooks"].append({"type": "command", "command": watch, "async": True, "timeout": 86400})
    (codex_home / "hooks.json").write_text(json.dumps({"hooks": hooks}))
    env = {key: value for key, value in os.environ.items() if not key.startswith("CODEX_")}
    env.update(CODEX_HOME=str(codex_home), HOME=str(home), USERPROFILE=str(home),
               LOFT_TELEMETRY="off", LOFT_UPDATE_CHECK="off")
    for name in ("OPENAI_API_KEY", "PLUGIN_DATA", "CLAUDE_PLUGIN_DATA", "CODEX_THREAD_ID"):
        env.pop(name, None)
    provider = ThreadingHTTPServer(("127.0.0.1", 0), Provider)
    Provider.home = home
    threading.Thread(target=provider.serve_forever, daemon=True).start()
    config = ('model_providers.fixture={name="Fixture",'
              f'base_url="http://127.0.0.1:{provider.server_port}/v1",'
              'wire_api="responses",requires_openai_auth=false,request_max_retries=0}')
    args = ["codex", "exec", "--dangerously-bypass-hook-trust", "--json",
            "-c", 'model_provider="fixture"', "-c", 'model="gpt-5.4"',
            "-c", config, "Complete the fixture plan."]
    with (home / "codex-contract-stderr.log").open("w+") as errors:
        process = subprocess.Popen(args, cwd=work, env=env, stdin=subprocess.DEVNULL,
                                   stdout=subprocess.PIPE, stderr=errors, text=True, bufsize=1)
        rows = queue.Queue()
        def read_rows():
            for line in process.stdout:
                rows.put(json.loads(line))
            rows.put({"closed": True})
        threading.Thread(target=read_rows, daemon=True).start()
        try:
            session = None
            while True:
                row = rows.get(timeout=20)
                if row.get("type") == "thread.started":
                    session = row["thread_id"]
                    stage = json.dumps([{"id": "contract", "kind": "stage", "title": "Contract", "status": "active"}])
                    subprocess.run([loft, "yard", "write", "--host", "codex", "--session", session,
                                    "--base", "main", "--input", "-", "--format", "json"], cwd=work,
                                   env=env, input=stage, text=True, check=True, capture_output=True)
                if row.get("closed"):
                    break
            assert process.wait(timeout=5) == 0
            assert session is not None
            document = json.loads(subprocess.check_output([
                loft, "yard", "read", "--host", "codex", "--session", session, "--format", "json"
            ], cwd=work, env=env))
            states = list((home / ".yard/codex").glob("*/state.json"))
            assert states, (document, "No native hook reached the adapter")
            state = json.loads(states[0].read_text())
            events = state["events"] + state["in_flight"]
            log = home / ".yard/events" / f"codex~{session}.jsonl"
            if log.exists():
                events += [json.loads(line) for line in log.read_text().splitlines()]
            assert any(event["kind"] == "tool.start" for event in events), events
            assert any(event["kind"] == "tool.end" for event in events), events
            assert Provider.requests == 2, Provider.requests
            print("Real Codex native tool hooks passed with loopback model fixtures.")
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
            provider.shutdown()
            errors.seek(0)
            diagnostics = errors.read()
            if diagnostics:
                print(diagnostics, file=sys.stderr)


if __name__ == "__main__":
    run(str(Path(sys.argv[1]).resolve()), Path(sys.argv[2]).resolve(), Path(sys.argv[3]).resolve())
