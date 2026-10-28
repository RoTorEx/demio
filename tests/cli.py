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
    env = dict(os.environ, DEMIO_CONFIG=str(root/'config.toml'), DEMIO_RUNTIME_DIR=str(root/'runtime'), DEMIO_HOME=str(root/'product'))
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
    assert listing[1]['project'] and listing[1]['created_at']
    created=json.loads(normal.read_text())['created_at']
    cli('register',str(normal))
    assert json.loads(normal.read_text())['created_at']==created
    # Explicit dates sort by actual instant, rather than list IDs or local clocks.
    older=manifest('a-older',['one']);data=json.loads(older.read_text());data.update(project='Archive',description='Earlier scenario',created_at='2026-01-01T12:00:00+01:00');older.write_text(json.dumps(data))
    newer=manifest('z-newer',['one']);data=json.loads(newer.read_text());data.update(project='Payments',description='Verify an approval result',created_at='2026-07-01T12:00:00+02:00');newer.write_text(json.dumps(data))
    cli('register',str(older));cli('register',str(newer))
    ordered=json.loads(cli('list','--json').stdout)
    positions={c['id']:n for n,c in enumerate(ordered)}
    assert positions['z-newer']<positions['a-older']
    offset=manifest('a-offset',['one']);data=json.loads(offset.read_text());data['created_at']='2026-07-01T11:00:00Z';offset.write_text(json.dumps(data));cli('register',str(offset))
    order=[item['id'] for item in json.loads(cli('list','--json').stdout)]
    assert order.index('a-offset')<order.index('z-newer')
    # Loading an old registry/manifest stays read-only and derives its project.
    legacy=manifest('legacy',['one']);legacy_config=root/'legacy.toml'
    legacy_text='theme = "calm"\n[channels.legacy]\nname = "Legacy"\nmanifest = '+json.dumps(str(legacy))+'\n'
    legacy_config.write_text(legacy_text)
    result=subprocess.run([binary,'list','--json'],env=dict(env,DEMIO_CONFIG=str(legacy_config)),capture_output=True,text=True,check=True)
    legacy_item=json.loads(result.stdout)[0]
    assert legacy_item['project']==root.name and legacy_item['available']
    assert legacy_config.read_text()==legacy_text and 'created_at' not in json.loads(legacy.read_text())
    item=next(c for c in ordered if c['id']=='z-newer')
    assert item['display_name']=='Payments · Test z-newer' and item['description']=='Verify an approval result'
    assert item['created_at']=='2026-07-01T12:00:00+02:00'
    output=cli('list').stdout
    assert 'Payments · Test z-newer' in output and 'Verify an approval result' in output
    assert 'CEST (UTC+02:00)' in output and 'CET (UTC+01:00)' in output
    # Renaming or describing an existing scenario does not change its creation date.
    data=json.loads(newer.read_text());data['name']='Renamed';data['description']='Changed';newer.write_text(json.dumps(data));cli('register',str(newer))
    assert json.loads(newer.read_text())['created_at']=='2026-07-01T12:00:00+02:00'
    newer.unlink()
    unavailable=next(c for c in json.loads(cli('list','--json').stdout) if c['id']=='z-newer')
    assert not unavailable['available'] and unavailable['project']=='Payments' and unavailable['description']=='Changed'
    invalid_date=manifest('invalid-date',['one']);data=json.loads(invalid_date.read_text());data['created_at']='yesterday';invalid_date.write_text(json.dumps(data));cli('validate',str(invalid_date),ok=False)

    alternate=root/'alternate.json';alternate.write_bytes(normal.read_bytes())
    cli('register',str(alternate),ok=False)
    cli('register',str(alternate),'--replace')
    cli('doctor','z-demo')
    cli('run','z-demo','--auto')
    log=(root/'actions.log').read_text()
    assert log.count('run:one') == 1 and log.count('run:two') == 1
    events=[json.loads(line) for line in (root/'product/logs/demio.jsonl').read_text().splitlines()]
    completed = [e for e in events if e.get('action') == 'scenario_run' and e.get('result') == 'success']
    assert completed and completed[-1]['fields']['completed'] == 2
    assert 'Read this' not in (root/'product/logs/demio.jsonl').read_text()
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
    # Diagnostics rotate without carrying narration; OS temporary files clean up.
    logfile=root/'product/logs/demio.jsonl'
    logfile.write_text('{"old":true}\n'*90000)
    temporary=root/'os-temp';temporary.mkdir()
    default_runtime=dict(env,TMPDIR=str(temporary));default_runtime.pop('DEMIO_RUNTIME_DIR',None)
    subprocess.run([binary,'run','z-demo','--auto'],check=True,env=default_runtime,capture_output=True,timeout=15)
    assert (root/'product/logs/demio.previous.jsonl').is_file()
    assert any(e.get('action')=='scenario_run' and e.get('result')=='success' for e in map(json.loads,logfile.read_text().splitlines()))
    assert not list((temporary/'demio').iterdir())
    assert not (root/'product/state').exists() and not (root/'product/sessions').exists()
    changed=json.loads(alternate.read_text());changed['id']='changed';alternate.write_text(json.dumps(changed))
    cli('run','z-demo','--auto',ok=False)
    assert not next(c for c in json.loads(cli('list','--json').stdout) if c['id']=='z-demo')['available']
    broken=root/'invalid.json';broken.write_text('{bad json');cli('validate',str(broken),ok=False)
    duplicate=manifest('duplicate',['same','same']);cli('validate',str(duplicate),ok=False)
    cli('unregister','z-demo');assert alternate.exists()
    cli('init',str(root/'example'));cli('validate',str(root/'example/demo.json'))
    example=json.loads((root/'example/scenarios/request/scenario.json').read_text());assert example['created_at'] and example['demo']=='../../demo.json' and example['description']
    assert (root/'example/AGENTS.md').is_file() and (root/'example/README.md').is_file()
    cli('register',str(root/'example/demo.json'))
    cli('init',str(root/'example'),ok=False)
    cli('theme','ocean');assert 'ocean *' in cli('themes').stdout
    cli('theme','unknown',ok=False)
print('CLI: registration, replacement, exact IDs, validation, sequential actions, failure and timeout boundaries, cleanup, config preservation passed.')
