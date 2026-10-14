"""Verify bundles before replacement, config preservation, and both executables."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import re
base=Path.home()/'construction_side/demio/checks';base.mkdir(parents=True,exist_ok=True)
project=Path(__file__).resolve().parents[1]
version=re.search(r'^version = "([^"]+)"',(project/'Cargo.toml').read_text(),re.M).group(1)
archive=Path(sys.argv[1]).resolve()/f'demio-v{version}-aarch64-apple-darwin.tar.gz'
installer=Path(__file__).resolve().parents[1]/'scripts/install.sh'
with tempfile.TemporaryDirectory(dir=base) as temporary:
    root=Path(temporary);install=root/'installed';config=root/'config.toml';config.write_text('theme = "ocean"\n')
    env=dict(os.environ,DEMIO_CONFIG=str(config))
    command=['sh',str(installer),'--archive',str(archive),'--install-dir',str(install),'--no-path-update']
    subprocess.run(command,check=True,env=env)
    version=subprocess.check_output([str(install/'bin/demio'),'--version'],text=True).strip()
    native=subprocess.check_output([str(install/'bin/demio-overlay'),'--version'],text=True).strip()
    assert version.split()[-1]==native.split()[-1]
    assert 'ocean' in subprocess.check_output([str(install/'bin/demio'),'config'],text=True,env=env)
    old=(install/'bin').resolve()
    bad=root/'bad.tar.gz';bad.write_bytes(archive.read_bytes()+b'bad')
    Path(str(bad)+'.sha256').write_bytes(Path(str(archive)+'.sha256').read_bytes())
    command[command.index('--archive')+1]=str(bad)
    assert subprocess.run(command,env=env,capture_output=True).returncode != 0
    assert (install/'bin').resolve()==old and config.read_text()=='theme = "ocean"\n'
print('Installer: native versions, checksum rejection, previous installation and config preservation passed.')
