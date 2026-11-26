"""Short AppKit smoke on a logged-in local macOS desktop."""
import json
import os
import shutil
from pathlib import Path
import subprocess
import sys
import tempfile
base=Path.home()/'construction_side/demio/checks';base.mkdir(parents=True,exist_ok=True)
with tempfile.TemporaryDirectory(dir=base) as temporary:
    root = Path(temporary)
    (root / 'panel.json').write_text(json.dumps({
        'status': 'Introduction', 'channel': 'Native check', 'scenario_id': 'native1', 'step': '0/5',
        'title': 'Introduction', 'say': 'This rehearsal uses the new presenter defaults.\n\nThe second topic has its own paragraph.',
        'detail': 'Start scenario prepares the demo.', 'wait': '', 'generation': 0,
        'primary': 'start', 'can_previous': False,
    }))
    result = subprocess.run([sys.argv[1], temporary, str(os.getpid()), '--smoke'],
                            text=True, capture_output=True, timeout=15,
                            env=dict(os.environ, DEMIO_CONFIG=str(root / 'config.toml'),
                                     DEMIO_HOME=temporary))
    assert result.returncode == 0, (result.returncode, result.stdout, result.stderr)
    assert result.stdout.splitlines() == ['start 1', 'confirm 4', 'restore 8', 'previous 12', 'next 12', 'quit']
    if len(sys.argv) > 2:
        shutil.copy2(root/'presenter.png', sys.argv[2])
    if len(sys.argv) > 3:
        shutil.copy2(root/'settings.png', sys.argv[3])
    if len(sys.argv) > 4:
        shutil.copy2(root/'off-route.png', sys.argv[4])

with tempfile.TemporaryDirectory(dir=base) as temporary:
    root=Path(temporary)
    (root/'config.toml').write_text('# Native config preservation\ntheme = "ocean"\n\n[channels.existing]\nname = "Example"\nmanifest = "/missing/scenario.json"\n')
    (root/'speaker-notes.json').write_text('{"font_size":26,"alignment":"center"}')
    (root/'panel.json').write_text(json.dumps({'status':'Ready','channel':'Native check','scenario_id':'native1','step':'2/5','title':'Read the speaker notes','say':'The presenter speaks. Notes and navigation move and resize together in one presenter window.','detail':'Ready','wait':'','generation':0,'primary':'next','can_previous':True}))
    result=subprocess.run([sys.argv[1],temporary,str(os.getpid()),'--smoke'],text=True,capture_output=True,timeout=15,env=dict(os.environ,DEMIO_CONFIG=str(root/'config.toml')))
    assert result.returncode==0,(result.returncode,result.stdout,result.stderr)
    assert result.stdout.splitlines() == ['start 1', 'confirm 4', 'restore 8', 'previous 12', 'next 12', 'quit']
    assert (root/'presenter.png').is_file()
    assert (root/'settings.png').is_file()
    text=(root/'config.toml').read_text()
    assert '# Native config preservation' in text and 'theme = "ocean"' in text and '[channels.existing]' in text
    assert '[overlay]' in text and 'font_size = 30.0' in text and 'alignment = "left"' in text
    assert not (root/'speaker-notes.json').exists()
    print(result.stderr.strip())
