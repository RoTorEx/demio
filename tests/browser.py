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
from playwright.sync_api import Error, sync_playwright
base=Path.home()/'construction_side/demio/checks';base.mkdir(parents=True,exist_ok=True)
root=Path(__file__).resolve().parents[1]
fixture=root/'tests/fixtures/browser'
with tempfile.TemporaryDirectory(dir=base) as temporary:
    with subprocess.Popen([sys.executable,'-u',str(fixture/'adapter.py')],cwd=fixture,stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True,env=dict(os.environ,DEMIO_HEADLESS='1')) as adapter:
        def call(sequence,op,step=None):
            adapter.stdin.write(json.dumps({'id':sequence,'op':op,'step':step,'session_dir':temporary})+'\n');adapter.stdin.flush()
            reply=json.loads(adapter.stdout.readline());assert reply['id']==sequence and reply.get('ok'),reply
        call(1,'check');call(2,'prepare')
        for i, step in enumerate(['intro','fill','submit','approve','close'],3): call(i,'run',step)
        call(8,'preview','fill');call(9,'preview','approve');call(10,'cleanup')
        adapter.wait(timeout=5);assert adapter.returncode==0
print('Browser: paced fields, scrolling, submit, approve, final state and read-only preview passed.')

binary = Path(sys.argv[1]).resolve()
with tempfile.TemporaryDirectory(prefix='browser scaffold ', dir=base) as temporary, sync_playwright() as observer:
    home = Path(temporary)
    demo = home / 'example demo'
    env = dict(os.environ, DEMIO_HOME=str(home), DEMIO_CONFIG=str(home / 'config.toml'),
               DEMIO_RUNTIME_DIR=str(home / 'runtime'),
               DEMIO_HEADLESS='0' if '--headed' in sys.argv[2:] else '1')
    # A loopback-only inspection port lets the test act as the presenter in the
    # same owned browser used by the real CLI. The shipped adapter is identical.
    inspection = home / 'inspection'
    inspection.mkdir()
    (inspection / 'sitecustomize.py').write_text('''from playwright.sync_api import BrowserType
launch = BrowserType.launch_persistent_context
def inspect(self, *args, **kwargs):
    kwargs["args"] = [*(kwargs.get("args") or []), "--remote-debugging-port=0",
                      "--remote-debugging-address=127.0.0.1"]
    return launch(self, *args, **kwargs)
BrowserType.launch_persistent_context = inspect
''')

    def cli(*args):
        result = subprocess.run([str(binary), *map(str, args)], env=env, capture_output=True,
                                text=True, timeout=90)
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
        assert document['type'] == ('system' if variant == 'mac-file' else 'browser')
        tokens = document['development_tokens']
        assert tokens['input'] >= tokens['cached_input'] > 0 and tokens['output'] > 0
        if variant != 'mac-file':
            cli('doctor', document['id'])
    cli('run', scenarios['navigation']['id'], '--auto')

    next_key = b'\x1b[20~'
    for variant in ('waiting', 'event', 'timeout', 'recovery'):
        document = scenarios['timeout' if variant == 'event' else variant]
        master, slave = pty.openpty()
        run_env = dict(env)
        if variant != 'waiting':
            run_env['PYTHONPATH'] = str(inspection)
        process = subprocess.Popen([binary, 'run', document['id'], '--no-overlay'],
                                   stdin=slave, stdout=slave, stderr=slave, env=run_env)
        os.close(slave)

        def expect(status, step, after=-1):
            deadline = time.monotonic() + 35
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
            if variant != 'waiting':
                debug_port = int((session / 'browser/DevToolsActivePort').read_text().splitlines()[0])
                chrome = observer.chromium.connect_over_cdp(f'http://127.0.0.1:{debug_port}')
                page = chrome.contexts[0].pages[0]
                assert page.url == 'https://www.wikipedia.org/'
                page.evaluate("""() => {
                    window.searchInputs = [];
                    window.moves = [];
                    document.addEventListener('input', event => window.searchInputs.push(event.target.value));
                    document.addEventListener('mousemove', event => window.moves.push([event.clientX, event.clientY]));
                }""")
            preceding_steps = 2 if variant == 'waiting' else 3
            for step in range(1, preceding_steps + 1):
                os.write(master, next_key)
                _, first = expect('Ready', step, after=first['generation'])
                if variant != 'waiting' and step == 1:
                    query = 'Ada Lovelace'
                    assert page.evaluate('searchInputs') == [query[:i] for i in range(1, len(query) + 1)]
                    assert len({tuple(position) for position in page.evaluate('moves')}) > 6
                    assert page.locator('[data-demio-pointer]').count() == 0
                    if variant == 'recovery':
                        stray = chrome.contexts[0].new_page()
                        _, drifted = expect('Route changed', 1)
                        assert drifted['primary'] == 'restore'
                        os.write(master, next_key)
                        assert expect('Route changed', 1)[1]['generation'] == drifted['generation']
                        os.write(master, b'r')
                        _, still_drifted = expect('Route changed', 1, after=drifted['generation'])
                        stray.close()
                        page.bring_to_front()
                        page.locator('#searchInput').click()
                        os.write(master, b'r')
                        _, first = expect('Ready', 1, after=still_drifted['generation'])
                        # Return on a fresh search document after leaving in the same tab.
                        page.goto('https://en.wikipedia.org/wiki/Wikipedia', wait_until='domcontentloaded')
                        _, wrong_page = expect('Route changed', 1, after=first['generation'])
                        assert wrong_page['detail'] == 'Return to Wikipedia search in the rehearsal tab.'
                        os.write(master, b'r')
                        _, still_wrong = expect('Route changed', 1, after=wrong_page['generation'])
                        page.goto('https://www.wikipedia.org/', wait_until='domcontentloaded')
                        field = page.locator('#searchInput')
                        assert field.input_value() == ''
                        os.write(master, b'r')
                        _, restoring = expect('Restoring route', 1, after=still_wrong['generation'])
                        assert restoring['primary'] is None and not restoring['can_previous']
                        os.write(master, next_key)
                        _, first = expect('Ready', 1, after=restoring['generation'])
                        assert first['primary'] == 'next' and page.url == 'https://www.wikipedia.org/'
                        assert field.input_value() == query
            expected_step = preceding_steps + 1
            os.write(master, next_key)
            if variant == 'event':
                _, waiting = expect('Waiting', expected_step)
                assert waiting['primary'] is None
                page.locator('a[href="#Work"]:visible').first.click()
                _, ready = expect('Ready', expected_step, after=waiting['generation'])
                assert page.url.endswith('/Ada_Lovelace#Work') and ready['primary'] == 'next'
            elif variant == 'timeout':
                expect('Waiting', expected_step)
                _, blocked = expect('Outcome unknown', expected_step)
                assert blocked['primary'] is None
                page.locator('a[href="#Work"]:visible').first.click()
                assert page.url.endswith('/Ada_Lovelace#Work')
                os.write(master, next_key * 3 + b'c')
                _, still_blocked = expect('Outcome unknown', expected_step)
                assert still_blocked['generation'] == blocked['generation']
            elif variant == 'waiting':
                expect('Waiting', expected_step)
                os.write(master, next_key * 3)
                expect('Ready', expected_step)
                download_step = expected_step + 1
                os.write(master, next_key)
                _, gate = expect('Awaiting confirmation', download_step)
                assert gate['primary'] == 'confirm'
                assert (session / 'tzdata2026f.tar.gz').is_file()
                os.write(master, next_key * 3)
                expect('Awaiting confirmation', download_step)
                os.write(master, b'c')
                expect('Ready', download_step)
                os.write(master, next_key)
                expect('Ready', download_step + 1)
                os.write(master, next_key)
                expect('Finished', download_step + 2)
            else:
                _, failed = expect('Step failed', expected_step)
                assert failed['primary'] == 'confirm' and 'Select Work' in failed['detail']
                os.write(master, next_key * 3)
                _, still_failed = expect('Step failed', expected_step)
                assert still_failed['generation'] == failed['generation']
                page.locator('a[href="#Work"]:visible').first.click()
                assert page.url.endswith('/Ada_Lovelace#Work')
                top = page.evaluate('scrollY')
                page.evaluate("""() => {
                    window.scrollPositions = [];
                    window.addEventListener('scroll', () => scrollPositions.push(scrollY), {passive: true});
                }""")
                os.write(master, b'c')
                expect('Ready', expected_step)
                os.write(master, next_key)
                expect('Ready', expected_step + 1)
                assert page.evaluate('scrollY') > top
                positions = page.evaluate('scrollPositions')
                assert len(set(positions)) >= 10, positions
                os.write(master, next_key)
                expect('Finished', expected_step + 2)
                try:
                    page.evaluate('location.href')
                except Error:
                    assert page.is_closed() or not chrome.is_connected()
                else:
                    raise AssertionError('Close left the rehearsal browser running.')
                os.write(master, b'\x1b[18~')
                expect('Finished', expected_step + 1)
                assert page.is_closed() or not chrome.is_connected()
            os.write(master, b'q')
            deadline = time.monotonic() + 8
            while process.poll() is None and time.monotonic() < deadline:
                if select.select([master], [], [], 0.1)[0]:
                    try:
                        os.read(master, 65536)
                    except OSError:
                        break
            assert (process.wait(timeout=1) == 0) == (variant != 'timeout'), variant
            print(f'Browser scaffold variant verified: {variant}')
        finally:
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=8)
            os.close(master)
    assert not list((home / 'runtime').iterdir())

print('Browser scaffold: visible pointer events, typed query, live tabs, countdown, official download, navigation event, confirmation, recovery, scrolling and close passed.')
