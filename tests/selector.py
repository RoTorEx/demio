"""Drive the real selector in a PTY; isolate adapters and stub only the native helper."""
import fcntl
import json
import os
from pathlib import Path
import pty
import re
import select
import shutil
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time

base = Path.home()/'construction_side/demio/checks'
base.mkdir(parents=True, exist_ok=True)
with tempfile.TemporaryDirectory(prefix="scenario with spaces ", dir=base) as temporary:
    root = Path(temporary)
    binary = root/'demio'
    shutil.copy2(sys.argv[1], binary)
    helper = root/'demio-overlay'
    helper.write_text('#!/bin/sh\nexec sleep 60\n')
    helper.chmod(0o755)
    adapter = root/'adapter.py'
    adapter.write_text('''import json,sys
from pathlib import Path
for line in sys.stdin:
 r=json.loads(line)
 with Path("actions.log").open("a") as log: log.write(r["op"]+":"+str(r["step"])+"\\n")
 print(json.dumps({"id":r["id"],"event":"done","ok":True}),flush=True)
 if r["op"]=="cleanup": break
''')
    env = dict(os.environ, TERM='xterm-256color', DEMIO_CONFIG=str(root/'config.toml'), DEMIO_RUNTIME_DIR=str(root/'runtime'), DEMIO_HOME=str(root/'product'))
    # Capture launch arguments without opening desktop apps during automated checks.
    launchers = root/'fake apps with spaces'
    launchers.mkdir()
    for name in ['code', 'open']:
        script = launchers/name
        script.write_text('#!'+sys.executable+'\nimport json,os,sys\nwith open(os.environ["DEMIO_LAUNCH_LOG"],"a") as log: log.write(json.dumps([sys.argv[0],*sys.argv[1:]])+"\\n")\nif os.environ.get("DEMIO_LAUNCH_FAIL"): sys.stderr.write("Launcher unavailable"); sys.exit(1)\n')
        script.chmod(0o755)
    env.update(PATH=str(launchers)+os.pathsep+os.environ['PATH'], DEMIO_LAUNCH_LOG=str(root/'launches.jsonl'))
    def start(rows=30, columns=140):
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', rows, columns, 0, 0))
        # Crossterm prefers the controlling terminal when measuring the window.
        # Use the sized test PTY even when the release command runs in a terminal.
        process = subprocess.Popen([str(binary)], stdin=slave, stdout=slave, stderr=slave,
                                   env=env, start_new_session=True)
        os.close(slave)
        return process, master
    def wait_text(master, expected):
        received = b''
        deadline = time.monotonic()+8
        while time.monotonic() < deadline:
            if select.select([master], [], [], .1)[0]:
                received += os.read(master, 65536)
                plain = re.sub(rb'\x1b\[[0-9;?]*[ -/]*[@-~]', b'', received)
                if re.sub(rb'\s+', b'', expected.encode()) in re.sub(rb'\s+', b'', plain):
                    return received
        raise AssertionError((expected, received[-3000:]))
    def wait_exit(process, master):
        deadline = time.monotonic()+5
        while process.poll() is None and time.monotonic() < deadline:
            if select.select([master], [], [], .05)[0]:
                try:
                    os.read(master, 65536)
                except OSError:
                    break
        return process.wait(timeout=1)
    def remaining_output(master):
        received = b''
        deadline = time.monotonic() + 1
        while time.monotonic() < deadline and select.select([master], [], [], .1)[0]:
            received += os.read(master, 65536)
        return received
    def finish(process, master):
        if process.poll() is None:
            process.kill()
            process.wait()
        os.close(master)
    # Empty state and cancel restore the terminal without invoking an adapter.
    process, master = start()
    try:
        wait_text(master, 'No demos registered')
        os.write(master, b'q')
        assert wait_exit(process, master) == 0
        assert not (root/'actions.log').exists()
    finally:
        finish(process, master)
    (root/'demo.json').write_text(json.dumps({'schema_version':1,'id':'example','name':'Example demo','description':'Shared selector components','scenarios':['scenarios/first/scenario.json','scenarios/second/scenario.json']}))
    for day, identity in [(1, 'first'), (2, 'second')]:
        channel = {'schema_version':1, 'demo':'../../demo.json', 'id':identity, 'name':identity.title()+' demonstration', 'project':'Example', 'description':'A local selector check', 'intro':f'Welcome to the {identity} scenario.', 'created_at':f'2026-01-0{day}T12:00:00+01:00', 'adapter':[sys.executable, '-u', str(adapter)], 'steps':[{'id':identity+'-step', 'title':identity+' step', 'role':'Presenter', 'say':'Read this', 'wait':'Wait for the visible result.', 'confirm':identity=='second'}]}
        manifest = root/f'scenarios/{identity}/scenario.json'
        manifest.parent.mkdir(parents=True, exist_ok=True)
        manifest.write_text(json.dumps(channel))
        subprocess.run([str(binary), 'register', str(manifest)], env=env, check=True, capture_output=True)
    process, master = start()
    try:
        preview = wait_text(master, 'Welcome to the second scenario.')
        plain = re.sub(rb'\x1b\[[0-9;?]*[ -/]*[@-~]', b'', preview)
        assert plain.index(b'Steps') < plain.index(b'Introduction')
        # A third-party adapter stays dormant while the cue and wait guidance are visible.
        assert not (root/'actions.log').exists()
        os.write(master, b'e')
        wait_text(master, 'Opened scenario folder')
        os.write(master, b'f')
        wait_text(master, 'in Finder')
        launches = [json.loads(line) for line in (root/'launches.jsonl').read_text().splitlines()]
        assert launches[0][1:] == ['--new-window', str(root), str(root/'scenarios/second/scenario.json')], launches
        assert launches[1][1:] == ['-R', str(root/'scenarios/second/scenario.json')], launches
        assert not (root/'actions.log').exists(), 'Opening sources must not execute adapters'
        # Resize while open, scroll details, then select the older demo by list position.
        fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack('HHHH', 24, 80, 0, 0))
        process.send_signal(signal.SIGWINCH)
        os.write(master, b'\x1b[6~\x1b[5~j\r')
        wait_text(master, 'Start scenario prepares the demo.')
        assert not (root/'actions.log').exists(), 'Selecting a scenario must leave its adapter dormant'
        os.write(master, b's')
        wait_text(master, 'Setup complete.')
        os.write(master, b'\x1b[20~')
        wait_text(master, 'Demo complete.')
        os.write(master, b'q')
        assert wait_exit(process, master) == 0
        actions = (root/'actions.log').read_text()
        assert actions.count('run:first-step') == 1 and 'run:second-step' not in actions, actions
        assert 'cleanup:' in actions and not list((root/'runtime').iterdir())
    finally:
        finish(process, master)
    env['DEMIO_LAUNCH_FAIL'] = '1'
    process, master = start()
    try:
        wait_text(master, 'Details')
        os.write(master, b'e')
        wait_text(master, 'Cannot open:')
        os.write(master, b'q')
        assert wait_exit(process, master) == 0
    finally:
        finish(process, master)
    del env['DEMIO_LAUNCH_FAIL']
    before = (root/'actions.log').read_text()
    (root/'scenarios/second/scenario.json').write_text('{broken')
    process, master = start()
    try:
        wait_text(master, 'Unavailable:')
        os.write(master, b'\r')
        wait_text(master, 'Cannot run:')
        os.write(master, b'\x1b')
        assert wait_exit(process, master) == 0
        assert (root/'actions.log').read_text() == before
    finally:
        finish(process, master)

    env['DEMIO_CONFIG'] = str(root/'wheel.toml')
    for number in range(18):
        identity = f'wheel-{number:02}'
        manifest = root/f'{identity}.json'
        manifest.write_text(json.dumps({
            'schema_version': 1, 'id': identity, 'name': chr(65 + number) * 6,
            'type': 'browser', 'project': 'Scrolling check',
            'created_at': '2026-01-01T10:00:00Z',
            'description': '\n'.join(
                'ZZZZZZZZ' if i == 7 else f'Right pane line {i:02}' for i in range(20)
            ),
            'adapter': [sys.executable, '-u', str(adapter)],
            'steps': [{'id': 'only-step', 'title': 'Check the wheel', 'role': 'Presenter', 'say': 'Read this'}],
        }))
        subprocess.run([str(binary), 'register', str(manifest)], env=env, check=True, capture_output=True)
    process, master = start(rows=15, columns=90)
    try:
        first_screen = wait_text(master, 'f Finder')
        assert b'\x1b[?1006h' in first_screen, 'Terminal mouse reporting must be enabled'
        os.write(master, b'\x1b[<65;70;7M' * 15)
        right_moved = wait_text(master, 'ZZZZZZZZ') + remaining_output(master)
        assert all(int(column) > 45 for column in re.findall(rb'\x1b\[\d+;(\d+)H', right_moved))
        os.write(master, b'\x1b[<65;10;7M' * 7)
        left_moved = wait_text(master, 'MMMMMM')
        assert all(int(column) <= 45 for column in re.findall(rb'\x1b\[\d+;(\d+)H', left_moved))
        os.write(master, b'f')
        wait_text(master, 'in Finder')
        assert json.loads((root/'launches.jsonl').read_text().splitlines()[-1])[-1] == str(root/'wheel-00.json')
        os.write(master, b'j')
        wait_text(master, 'Metadata')
        os.write(master, b'q')
        assert wait_exit(process, master) == 0
    finally:
        finish(process, master)
print('Selector: source opening, exact launcher arguments, launch failure, empty state, resize, navigation, independent wheel scrolling, selected launch, unavailable guard and cleanup passed.')
