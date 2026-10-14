"""Exercise the actual installed entrypoint with isolated config and adapters."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import pty
import select
import time

binary = str(Path(sys.argv[1]).resolve())
base = Path.home()/'construction_side/demio/checks'
base.mkdir(parents=True, exist_ok=True)
with tempfile.TemporaryDirectory(dir=base) as temporary:
    root = Path(temporary)
    env = dict(os.environ, DEMIO_CONFIG=str(root/'config.toml'), DEMIO_RUNTIME_DIR=str(root/'runtime'), XDG_STATE_HOME=str(root/'state'))
    def cli(*args, ok=True):
        result = subprocess.run([binary,*args], env=env, text=True, capture_output=True, timeout=15)
        assert (result.returncode == 0) == ok, (args,result.stdout,result.stderr)
        return result
    adapter = root/'adapter.py'
    adapter.write_text('''import json,sys,time
from pathlib import Path
for line in sys.stdin:
 r=json.loads(line)
 with Path("actions.log").open("a") as stream: stream.write(r["op"]+":"+str(r["step"])+"\\n")
 if r["op"]=="run" and r["step"]=="fail": print(json.dumps({"id":r["id"],"event":"done","ok":False,"error":"intentional failure"}),flush=True)
 elif r["op"]=="run" and r["step"]=="timeout": time.sleep(3)
 elif r["op"]=="run" and r["step"]=="wrong": print(json.dumps({"id":999,"event":"done","ok":True}),flush=True)
 else: print(json.dumps({"id":r["id"],"event":"done","ok":True}),flush=True)
 if r["op"]=="cleanup": break
''')
    def manifest(identity, steps):
        data={'schema_version':1,'id':identity,'name':'Test '+identity,'workspace':'.','adapter':[sys.executable,'-u','adapter.py'],'steps':[{'id':s,'title':s,'role':'Presenter','say':'Read this','timeout_seconds':1} for s in steps]}
        path=root/(identity+'.json');path.write_text(json.dumps(data));return path
    normal=manifest('z-demo',['one','two'])
    cli('register',str(normal));cli('register',str(manifest('a-demo',['one'])))
    listing=json.loads(cli('list','--json').stdout)
    assert [c['id'] for c in listing] == ['a-demo','z-demo']
    assert listing[1]['name']=='Test z-demo'
    alternate=root/'alternate.json';alternate.write_bytes(normal.read_bytes())
    cli('register',str(alternate),ok=False)
    cli('register',str(alternate),'--replace')
    cli('doctor','z-demo')
    cli('run','z-demo','--auto')
    log=(root/'actions.log').read_text()
    assert log.count('run:one') == 1 and log.count('run:two') == 1
    assert json.loads((root/'state/demio/last-session.json').read_text())['completed'] == 2
    # Exercise the actual interactive entrypoint, including backward navigation.
    (root/'actions.log').write_text('')
    master, slave = pty.openpty()
    interactive = subprocess.Popen([binary,'run','z-demo','--no-overlay'],stdin=slave,stdout=slave,stderr=slave,env=dict(env,TERM='xterm-256color'))
    os.close(slave)
    def wait_text(expected):
        received=b'';deadline=time.monotonic()+8
        while time.monotonic()<deadline:
            if select.select([master],[],[],0.1)[0]:
                received+=os.read(master,65536)
                if expected.encode() in received: return
        raise AssertionError((expected,received))
    try:
        wait_text('Press F9 to begin')
        os.write(master,b'\r');wait_text('Step complete.')
        os.write(master,b'\r');wait_text('Demo complete.')
        os.write(master,b'\x1b[18~');wait_text('Viewing completed step')
        wait_text('Demo complete.')
        os.write(master,b'\r');wait_text('Viewing completed step')
        wait_text('Demo complete.')
        os.write(master,b'q');interactive.wait(timeout=5)
        assert interactive.returncode==0
        actions=(root/'actions.log').read_text()
        assert actions.count('run:one')==1 and actions.count('run:two')==1,actions
        assert 'preview:one' in actions and 'preview:two' in actions,actions
    finally:
        if interactive.poll() is None: interactive.kill();interactive.wait()
        os.close(master)
    for failure in ['fail','timeout','wrong']:
        cli('register',str(manifest(failure,[failure,'never'])))
        result=cli('run',failure,'--auto',ok=False)
        assert 'run:never' not in (root/'actions.log').read_text()
        assert not list((root/'runtime').iterdir()), 'Owned session scratch leaked'
    changed=json.loads(alternate.read_text());changed['id']='changed';alternate.write_text(json.dumps(changed))
    cli('run','z-demo','--auto',ok=False)
    assert not next(c for c in json.loads(cli('list','--json').stdout) if c['id']=='z-demo')['available']
    broken=root/'invalid.json';broken.write_text('{bad json');cli('validate',str(broken),ok=False)
    duplicate=manifest('duplicate',['same','same']);cli('validate',str(duplicate),ok=False)
    cli('unregister','z-demo');assert alternate.exists()
    cli('init',str(root/'example'));cli('validate',str(root/'example/channel.json'))
    cli('init',str(root/'example'),ok=False)
    cli('theme','ocean');assert 'ocean *' in cli('themes').stdout
    cli('theme','unknown',ok=False)
print('CLI: registration, replacement, exact IDs, validation, sequential actions, failure and timeout boundaries, cleanup, config preservation passed.')
