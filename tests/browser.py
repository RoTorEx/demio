"""Run with the example's Playwright Python and the built Demio CLI as the argument."""
import json
import os
from pathlib import Path
import pty
import select
import subprocess
import sys
import tempfile
import time
base=Path.home()/'construction_side/demio/checks';base.mkdir(parents=True,exist_ok=True)
root=Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(dir=base) as temporary:
    with subprocess.Popen([sys.executable,'-u',str(root/'examples/browser/adapter.py')],cwd=root/'examples/browser',stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True,env=dict(os.environ,DEMIO_HEADLESS='1')) as adapter:
        def call(sequence,op,step=None):
            adapter.stdin.write(json.dumps({'id':sequence,'op':op,'step':step,'session_dir':temporary})+'\n');adapter.stdin.flush()
            reply=json.loads(adapter.stdout.readline());assert reply['id']==sequence and reply.get('ok'),reply
        call(1,'check');call(2,'prepare')
        for i, step in enumerate(['intro','fill','submit','approve','close'],3): call(i,'run',step)
        call(8,'preview','fill');call(9,'preview','approve');call(10,'cleanup')
        adapter.wait(timeout=5);assert adapter.returncode==0
print('Browser: paced fields, scrolling, submit, approve, final state and read-only preview passed.')

binary = Path(sys.argv[1]).resolve()
with tempfile.TemporaryDirectory(prefix='browser scaffold ', dir=base) as temporary:
    home = Path(temporary)
    demo = home / 'example demo'
    env = dict(os.environ, DEMIO_HOME=str(home), DEMIO_CONFIG=str(home / 'config.toml'),
               DEMIO_RUNTIME_DIR=str(home / 'runtime'), DEMIO_HEADLESS='1')

    def cli(*args):
        result = subprocess.run([str(binary), *map(str, args)], env=env, capture_output=True,
                                text=True, timeout=60)
        assert result.returncode == 0, (args, result.stdout, result.stderr)

    cli('init', demo)
    manifest = demo / 'demo.json'
    definition = json.loads(manifest.read_text())
    # Reuse the interpreter with Playwright available. The scaffold still launches from its demo root.
    definition['adapter'][0] = sys.executable
    manifest.write_text(json.dumps(definition))
    cli('validate', manifest)
    cli('register', manifest)
    scenarios = {}
    for path in definition['scenarios']:
        document = json.loads((demo / path).read_text())
        variant = Path(path).parent.name
        scenarios[variant] = document
        assert len(document['steps']) >= 3 and document['intro'] != document['steps'][0]['say']
        if variant != 'mac-file':
            cli('doctor', document['id'])
    cli('run', scenarios['navigation']['id'], '--auto')

    next_key = b'\x1b[20~'
    for variant in ('waiting', 'timeout', 'recovery'):
        document = scenarios[variant]
        master, slave = pty.openpty()
        process = subprocess.Popen([binary, 'run', document['id'], '--no-overlay'],
                                   stdin=slave, stdout=slave, stderr=slave, env=env)
        os.close(slave)

        def expect(status, step, after=-1):
            deadline = time.monotonic() + 15
            frame = None
            while time.monotonic() < deadline:
                if select.select([master], [], [], 0.1)[0]:
                    os.read(master, 65536)
                panel = next((home / 'runtime').glob('*/panel.json'), None)
                if panel:
                    frame = json.loads(panel.read_text())
                    if (frame['status'] == status and frame['step'].split('/')[0] == str(step)
                            and frame['generation'] > after):
                        return panel.parent, frame
            raise AssertionError((variant, status, step, frame))

        try:
            session, intro = expect('Introduction', 0)
            assert intro['say'] == document['intro'] and not (session / 'browser').exists()
            os.write(master, b's')
            _, first = expect('Ready', 1)
            assert first['say'] == document['steps'][0]['say']
            os.write(master, next_key)
            expect('Ready', 1, after=first['generation'])
            os.write(master, next_key)
            if variant == 'timeout':
                expect('Waiting', 2)
                _, blocked = expect('Outcome unknown', 2)
                assert blocked['primary'] is None
                os.write(master, next_key * 3 + b'c')
                _, still_blocked = expect('Outcome unknown', 2)
                assert still_blocked['generation'] == blocked['generation']
            elif variant == 'waiting':
                expect('Waiting', 2)
                os.write(master, next_key * 3)
                expect('Ready', 2)
                os.write(master, next_key)
                _, gate = expect('Awaiting confirmation', 3)
                assert gate['primary'] == 'confirm'
                assert (session / 'tzdata2026f.tar.gz').is_file()
                os.write(master, next_key * 3)
                expect('Awaiting confirmation', 3)
                os.write(master, b'c')
                expect('Ready', 3)
                os.write(master, next_key)
                expect('Finished', 4)
            else:
                _, failed = expect('Step failed', 2)
                assert failed['primary'] == 'confirm' and 'Select Work' in failed['detail']
            os.write(master, b'q')
            deadline = time.monotonic() + 8
            while process.poll() is None and time.monotonic() < deadline:
                if select.select([master], [], [], 0.1)[0]:
                    try:
                        os.read(master, 65536)
                    except OSError:
                        break
            assert (process.wait(timeout=1) == 0) == (variant == 'waiting'), variant
            print(f'Browser scaffold variant verified: {variant}')
        finally:
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=8)
            os.close(master)
    assert not list((home / 'runtime').iterdir())
print('Browser scaffold: live tabs, countdown, official download, confirmation, real prerequisite failure and timeout passed.')
