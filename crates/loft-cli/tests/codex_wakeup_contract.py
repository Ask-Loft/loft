"""Cross-client native TUI experiment, pinned to Codex 0.164.0; loopback model only."""
import importlib.util
import json
import os
import pty
import queue
import select
import shutil
import signal
import struct
import subprocess
import sys
import tempfile
import threading
import time
import fcntl
import termios
from pathlib import Path
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

SOURCE = Path(__file__).with_name('codex_app_server_contract.py')
spec = importlib.util.spec_from_file_location('peer', SOURCE)
peer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(peer)

class Rpc(peer.Client):
    def __init__(self, path):
        self.notices = []
        super().__init__(path)
    def call(self, method, params):
        self.number += 1
        number = self.number
        self.send({'id':number, 'method':method, 'params':params})
        while True:
            row = self.receive()
            if row.get('id') == number:
                return row
            self.notices.append(row)
    def ok(self, method, params):
        row = self.call(method, params)
        assert 'result' in row, (method, row)
        return row['result']
    def until(self, method, predicate=lambda row: True, timeout=15):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            for i, row in enumerate(self.notices):
                if row.get('method') == method and predicate(row):
                    return self.notices.pop(i)
            self.connection.settimeout(max(.01, deadline-time.monotonic()))
            self.notices.append(self.receive())
        raise AssertionError(('missing notification', method, self.notices[-10:]))

class Provider(BaseHTTPRequestHandler):
    seen = queue.Queue()
    gates = queue.Queue()
    counter = 0
    automatic = False
    def log_message(self, *_args): pass
    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        auxiliary = 'Generate a concise, single-line task title' in json.dumps(body)
        if not auxiliary: Provider.counter += 1
        number = 0 if auxiliary else Provider.counter
        (Provider.root / f'request-{number}.json').write_text(json.dumps(body,indent=2))
        gate = threading.Event()
        if auxiliary or Provider.automatic: gate.set()
        else: Provider.seen.put((number, body, gate))
        assert gate.wait(20), 'test failed to release fixture request'
        item = {'id':f'message_{number}', 'type':'message', 'role':'assistant',
                'content':[{'type':'output_text','text':f'Fixture response {number}.'}]}
        rows = [
          {'type':'response.output_item.done','output_index':0,'item':item},
          {'type':'response.completed','response':{'id':f'fixture_{number}','status':'completed','output':[item],
            'usage':{'input_tokens':100,'output_tokens':5,'total_tokens':105}}}]
        try:
            self.send_response(200)
            self.send_header('Content-Type','text/event-stream')
            self.end_headers()
            for row in rows:
                self.wfile.write(('data: '+json.dumps(row)+'\n\n').encode())
                self.wfile.flush()
        except (BrokenPipeError, ConnectionResetError): pass

def wait_for(predicate, message, timeout=12):
    deadline = time.monotonic()+timeout
    while time.monotonic()<deadline:
        value = predicate()
        if value:return value
        time.sleep(.04)
    raise AssertionError(message)

def run(root, controller=False):
    version=subprocess.check_output(['codex','--version'],text=True).strip()
    assert version=='codex-cli 0.164.0',version
    root.mkdir(exist_ok=True)
    work=root/'work'; home=root/'home'; ch=home/'.codex'
    work.mkdir(exist_ok=True); ch.mkdir(parents=True,exist_ok=True)
    Provider.root=root
    provider=ThreadingHTTPServer(('127.0.0.1',0),Provider)
    provider.daemon_threads=True
    threading.Thread(target=provider.serve_forever,daemon=True).start()
    (ch/'config.toml').write_text('\n'.join([
      'model="gpt-5.4"','model_provider="fixture"','check_for_update_on_startup=false',
      'approval_policy="never"','sandbox_mode="read-only"','[analytics]','enabled=false',
      '[model_providers.fixture]','name="Fixture"',
      f'base_url="http://127.0.0.1:{provider.server_port}/v1"','wire_api="responses"',
      'requires_openai_auth=false','request_max_retries=0',
      '[projects.'+json.dumps(str(work))+']','trust_level="trusted"'])+'\n')
    env={k:v for k,v in os.environ.items() if not k.startswith(('CODEX_','GIT_')) and not k.endswith('API_KEY')}
    env.update(HOME=str(home),USERPROFILE=str(home),CODEX_HOME=str(ch),TERM='xterm-256color',
      LOFT_TELEMETRY='off',LOFT_UPDATE_CHECK='off',GIT_CONFIG_NOSYSTEM='1',GIT_CONFIG_GLOBAL=os.devnull)
    endpoint=root/'server.sock'
    processes=[];clients=[]; gates=[]; master=None; results=[]
    def report(case, **data):
        row={'case':case,**data};results.append(row);print(json.dumps(row),flush=True)
        (root/'results.json').write_text(json.dumps(results,indent=2)+'\n')
    try:
        errors=(root/'server-errors.log').open('w')
        server=subprocess.Popen([shutil.which('codex'),'app-server','--listen','unix://'+str(endpoint)],
            cwd=work,env=env,stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=errors,start_new_session=True)
        processes.append(server)
        wait_for(endpoint.exists,'no server socket')
        monitor=Rpc(endpoint); clients.append(monitor)
        master,slave=pty.openpty()
        fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',42,160,0,0))
        tui=subprocess.Popen([shutil.which('codex'),'--remote','unix://'+str(endpoint),'--no-alt-screen',
            '-C',str(work),'-a','never','-s','read-only','Baseline fixture.'],
            cwd=work,env=env,stdin=slave,stdout=slave,stderr=slave,start_new_session=True)
        os.close(slave); processes.append(tui)
        terminal=bytearray(); finished=threading.Event()
        def drain():
            while not finished.is_set():
                if not select.select([master],[],[],.1)[0]:continue
                try:data=os.read(master,65536)
                except OSError:break
                if not data:break
                terminal.extend(data)
                (root/'terminal.log').write_bytes(terminal)
                if b'\x1b[6n' in data:os.write(master,b'\x1b[1;1R')
                if b'\x1b[c' in data:os.write(master,b'\x1b[?1;2c')
        threading.Thread(target=drain,daemon=True).start()
        try:number,body,gate=Provider.seen.get(timeout=15)
        except queue.Empty:
            raise AssertionError(('TUI never requested fixture',tui.poll(),bytes(terminal)[-6000:]))
        gates.append(gate);gate.set()
        thread=wait_for(lambda:monitor.ok('thread/loaded/list',{}).get('data'), 'TUI thread not loaded')[0]
        monitor.ok('thread/resume',{'threadId':thread,'excludeTurns':True})
        # Native client remains connected and renders the completed baseline.
        wait_for(lambda:b'Fixture response 1.' in terminal,'TUI did not render baseline')
        wait_for(lambda:monitor.ok('thread/read',{'threadId':thread})['thread']['status']['type']=='idle','baseline did not finish')
        if controller:
            Provider.automatic=True
            print(json.dumps({'socket':str(endpoint),'thread':thread,'directory':str(work)}),flush=True)
            control(monitor, thread, master, terminal, gates)
            return
        report('baseline',passed=True,version=version,transport='unix-websocket',client='native-tui',thread=thread,tui_alive=tui.poll() is None,fixture_requests=number)
        # External idle wakeup without pressing a key in the TUI.
        idle=monitor.ok('turn/start',{'threadId':thread,'input':[{'type':'text','text':'YARD_WAKE_IDLE'}]})
        turn=idle['turn']['id']
        number,body,gate=Provider.seen.get(timeout=12);gates.append(gate)
        assert 'YARD_WAKE_IDLE' in json.dumps(body), body
        gate.set()
        done=monitor.until('turn/completed',lambda row:row['params']['turn']['id']==turn)
        wait_for(lambda:b'Fixture response 2.' in terminal,'TUI missed external wakeup response')
        report('external_idle_wakeup',passed=True,turn=turn,status=done['params']['turn']['status'],tui_alive=tui.poll() is None)
        # Hold a native model request in-flight while the second client steers.
        begun=monitor.ok('turn/start',{'threadId':thread,'input':[{'type':'text','text':'YARD_ACTIVE_BASE'}]})
        active=begun['turn']['id']
        number,body,gate=Provider.seen.get(timeout=12);gates.append(gate)
        stale=monitor.call('turn/steer',{'threadId':thread,'expectedTurnId':'stale-turn','input':[{'type':'text','text':'YARD_MUST_NOT_ENTER'}]})
        assert 'error' in stale,stale
        report('stale_turn_precondition',passed=True,error=stale['error'])
        steered=monitor.ok('turn/steer',{'threadId':thread,'expectedTurnId':active,'input':[{'type':'text','text':'YARD_STEER_ACTIVE'}]})
        assert steered['turnId']==active,steered
        gate.set()
        number,body,gate=Provider.seen.get(timeout=12);gates.append(gate)
        assert 'YARD_STEER_ACTIVE' in json.dumps(body),body
        assert 'YARD_MUST_NOT_ENTER' not in json.dumps(body),body
        gate.set()
        monitor.until('turn/completed',lambda row:row['params']['turn']['id']==active)
        report('external_active_steer',passed=True,turn=active,followup_request=number,tui_alive=tui.poll() is None)
        # Once idle, steer must refuse rather than silently starting work.
        no_active=monitor.call('turn/steer',{'threadId':thread,'expectedTurnId':active,'input':[{'type':'text','text':'YARD_IDLE_STEER_REJECT'}]})
        assert 'error' in no_active,no_active
        report('idle_steer_refused',passed=True,error=no_active['error'])
        # A second client racing a user turn must not run a parallel model turn.
        begun=monitor.ok('turn/start',{'threadId':thread,'input':[{'type':'text','text':'YARD_RACE_BASE'}]})
        racing=begun['turn']['id']
        number,body,gate=Provider.seen.get(timeout=12);gates.append(gate)
        racer=Rpc(endpoint);clients.append(racer)
        racer.ok('thread/resume',{'threadId':thread,'excludeTurns':True})
        joined=racer.ok('turn/start',{'threadId':thread,'input':[{'type':'text','text':'YARD_RACE_NOTIFICATION'}]})
        assert joined['turn']['id']==racing,joined
        gate.set()
        number,body,gate=Provider.seen.get(timeout=12);gates.append(gate)
        assert 'YARD_RACE_NOTIFICATION' in json.dumps(body)
        gate.set()
        monitor.until('turn/completed',lambda row:row['params']['turn']['id']==racing)
        report('turn_start_race_joins_active_turn',passed=True,turn=racing,tui_alive=tui.poll() is None)
        # Native TUI escape explicitly interrupts the turn, independent of the monitor.
        begun=monitor.ok('turn/start',{'threadId':thread,'input':[{'type':'text','text':'YARD_INTERRUPT_BASE'}]})
        interrupted=begun['turn']['id']
        number,body,gate=Provider.seen.get(timeout=12);gates.append(gate)
        # Wait for the TUI to render this native turn as active before pressing Escape.
        wait_for(lambda:b'YARD_INTERRUPT_BASE' in terminal,'TUI did not show interrupt target')
        os.write(master,b'\x1b')
        ended=monitor.until('turn/completed',lambda row:row['params']['turn']['id']==interrupted)
        assert ended['params']['turn']['status']=='interrupted',ended
        gate.set()
        stopped=monitor.call('turn/steer',{'threadId':thread,'expectedTurnId':interrupted,'input':[{'type':'text','text':'YARD_NO_RESTART_VIA_STEER'}]})
        assert 'error' in stopped,stopped
        assert tui.poll() is None
        report('native_tui_interrupt',passed=True,status='interrupted',subsequent_steer_refused=True,tui_alive=True)
        # turn/start is still a start request: cancellation policy must be enforced by Yard.
        after=monitor.ok('turn/start',{'threadId':thread,'input':[{'type':'text','text':'YARD_AFTER_INTERRUPT'}]})
        assert after['turn']['id']!=interrupted,after
        number,body,gate=Provider.seen.get(timeout=12);gates.append(gate)
        assert 'YARD_AFTER_INTERRUPT' in json.dumps(body)
        gate.set()
        monitor.until('turn/completed',lambda row:row['params']['turn']['id']==after['turn']['id'])
        report('turn_start_can_restart_after_interrupt',passed=True,requires_adapter_cancellation_gate=True)
        # Reconnect a monitor without moving the conversation to a separate Codex process.
        monitor.connection.close()
        clients.remove(monitor)
        reconnect=Rpc(endpoint);clients.append(reconnect)
        loaded=reconnect.ok('thread/loaded/list',{})
        assert thread in loaded['data'],loaded
        reconnect.ok('thread/resume',{'threadId':thread,'excludeTurns':True})
        resumed=reconnect.ok('turn/start',{'threadId':thread,'input':[{'type':'text','text':'YARD_RECONNECT'}]})
        number,body,gate=Provider.seen.get(timeout=12);gates.append(gate)
        assert 'YARD_RECONNECT' in json.dumps(body)
        gate.set()
        reconnect.until('turn/completed',lambda row:row['params']['turn']['id']==resumed['turn']['id'])
        monitor=reconnect
        report('monitor_reconnect',passed=True,thread=thread,previous_connection_closed=True,tui_alive=tui.poll() is None)
        # A repeated delivery id is correlation metadata, not assumed idempotency.
        duplicate_id='msg_yard_contract_duplicate'
        duplicated=monitor.ok('turn/start',{'threadId':thread,'clientUserMessageId':duplicate_id,'input':[{'type':'text','text':'YARD_DUPLICATE_EVENT'}]})
        number,body,gate=Provider.seen.get(timeout=12);gates.append(gate)
        again=monitor.ok('turn/start',{'threadId':thread,'clientUserMessageId':duplicate_id,'input':[{'type':'text','text':'YARD_DUPLICATE_EVENT'}]})
        assert again['turn']['id']==duplicated['turn']['id'],again
        gate.set()
        number,body,gate=Provider.seen.get(timeout=12);gates.append(gate)
        copies=sum(1 for item in body['input'] if item.get('role')=='user' and any(part.get('text')=='YARD_DUPLICATE_EVENT' for part in item.get('content',[])))
        assert copies==2,('native duplicate-delivery contract changed',copies)
        gate.set()
        monitor.until('turn/completed',lambda row:row['params']['turn']['id']==duplicated['turn']['id'])
        wait_for(lambda:f'Fixture response {number}.'.encode() in terminal,'TUI missed final fixture response')
        report('repeated_client_message_id',passed=True,copies_in_model_context=copies,native_deduplicated=copies==1)
        for request in root.glob('request-*.json'):
            text=request.read_text()
            assert 'read-only' in text and 'Approval policy is currently never.' in text,request
        report('native_permissions_preserved',passed=True,sandbox='read-only',approval_policy='never')
        report('success',passed=True,primary_requests=Provider.counter)
    finally:
        for gate in gates:gate.set()
        if 'finished' in locals():finished.set()
        for client in clients:client.connection.close()
        for process in reversed(processes):
            if process.poll() is None:
                os.killpg(process.pid,signal.SIGTERM)
                try:process.wait(timeout=4)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid,signal.SIGKILL);process.wait()
        if master is not None:os.close(master)
        provider.shutdown();provider.server_close()

def control(owner, thread, master, terminal, gates):
    for line in sys.stdin:
        command=json.loads(line)
        action=command['action']
        if action=='active':
            Provider.automatic=False
            active_text=f'NATIVE_OWNER_ACTIVE_{Provider.counter+1}'
            started=owner.ok('turn/start',{'threadId':thread,'input':[{'type':'text','text':active_text}]})
            number,body,gate=Provider.seen.get(timeout=10)
            gates.append(gate)
            print(json.dumps({'turn':started['turn']['id']}),flush=True)
        elif action in ('settle','interrupt'):
            if action=='interrupt':
                wait_for(lambda:active_text.encode() in terminal,'TUI did not display active turn')
                os.write(master,b'\x1b')
            else:
                Provider.automatic=True
                for gate in gates:gate.set()
            ended=owner.until('turn/completed',lambda row:row['params']['turn']['id']==command['turn'])
            Provider.automatic=True
            for gate in gates:gate.set()
            print(json.dumps({'status':ended['params']['turn']['status']}),flush=True)
        else:
            raise AssertionError(('unknown control action',action))

if __name__=='__main__':
    root=Path(tempfile.mkdtemp(prefix='cw-',dir='/tmp'))
    try:
        run(root, controller='--controller' in sys.argv)
    except BaseException:
        print('Native fixture evidence: '+str(root),file=sys.stderr)
        raise
    else:
        shutil.rmtree(root)

