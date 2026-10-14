"""Verify the browser adapter's real visible state and read-only preview."""
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
