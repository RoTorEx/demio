"""Short AppKit smoke on a logged-in local macOS desktop."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
base=Path.home()/'construction_side/demio/checks';base.mkdir(parents=True,exist_ok=True)
with tempfile.TemporaryDirectory(dir=base) as temporary:
    root=Path(temporary)
    (root/'panel.json').write_text(json.dumps({'status':'Ready','channel':'Native check','step':'2/5','role':'Requester','title':'Read the speaker notes','say':'The presenter speaks. The notes window moves and resizes independently from the two-button remote.','detail':'Ready'}))
    result=subprocess.run([sys.argv[1],temporary,str(os.getpid()),'--smoke'],text=True,capture_output=True,timeout=15)
    assert result.returncode==0,(result.stdout,result.stderr)
    assert result.stdout.splitlines()==['previous','next']
    assert (root/'notes.png').is_file() and (root/'remote.png').is_file()
    print(result.stderr.strip())
