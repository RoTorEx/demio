"""Run with the example's Playwright Python and the built Demio CLI as the argument."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
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
    for path in definition['scenarios']:
        scenario = json.loads((demo / path).read_text())['id']
        cli('doctor', scenario)
        cli('run', scenario, '--auto')
    assert not list((home / 'runtime').iterdir())
print('Browser scaffold: both generated walkthroughs ran from a relocated demo root and cleaned up.')
