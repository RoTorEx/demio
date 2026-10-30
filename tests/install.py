"""Verify bundles before replacement, config preservation, and both executables."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import re
import shutil
base=Path.home()/'construction_side/demio/checks';base.mkdir(parents=True,exist_ok=True)
project=Path(__file__).resolve().parents[1]
version=re.search(r'^version = "([^"]+)"',(project/'demio-cli/Cargo.toml').read_text(),re.M).group(1)
archive=Path(sys.argv[1]).resolve()/f'demio-v{version}-aarch64-apple-darwin.tar.gz'
installer=Path(__file__).resolve().parents[1]/'scripts/install.sh'
with tempfile.TemporaryDirectory(dir=base) as temporary:
    root=Path(temporary);install=root/'installed';install.mkdir();config=install/'config.toml';config.write_text('theme = "ocean"\n')
    env=dict(os.environ,DEMIO_HOME=str(install));env.pop('DEMIO_CONFIG',None)
    command=['sh',str(installer),'--archive',str(archive),'--install-dir',str(install),'--no-path-update']
    subprocess.run(command,check=True,env=env)
    version=subprocess.check_output([str(install/'bin/demio'),'--version'],text=True).strip()
    native=subprocess.check_output([str(install/'bin/demio-overlay'),'--version'],text=True).strip()
    assert version.split()[-1]==native.split()[-1]
    assert 'ocean' in subprocess.check_output([str(install/'bin/demio'),'config'],text=True,env=env)
    assert not (install/'bin').is_symlink() and not (install/'versions').exists()
    demo=install/'demos/example/demo.json';demo.parent.mkdir();demo.write_text('{"id":"stable"}')
    log=install/'logs/keep.log';log.write_text('keep')
    # Exercise replacement of both the new layout and the old symlink layout.
    subprocess.run(command,check=True,env=env)
    (install/'versions').mkdir();(install/'bin').rename(install/'versions/old')
    (install/'bin').symlink_to(install/'versions/old')
    subprocess.run(command,check=True,env=env)
    assert not (install/'versions').exists() and not (install/'bin').is_symlink()
    assert demo.read_text()=='{"id":"stable"}' and log.read_text()=='keep'
    assert config.read_text()=='theme = "ocean"\n'
    old=(install/'bin/demio').read_bytes()
    bad=root/'bad.tar.gz';bad.write_bytes(archive.read_bytes()+b'bad')
    Path(str(bad)+'.sha256').write_bytes(Path(str(archive)+'.sha256').read_bytes())
    command[command.index('--archive')+1]=str(bad)
    assert subprocess.run(command,env=env,capture_output=True).returncode != 0
    assert (install/'bin/demio').read_bytes()==old and config.read_text()=='theme = "ocean"\n'
    # Default home imports legacy preferences once and rewrites PATH once.
    home=root/'home';legacy=home/'.config/demio';legacy.mkdir(parents=True)
    (legacy/'config.toml').write_text('theme = "forest"\n')
    (legacy/'speaker-notes.json').write_text('{"font_size":28}')
    (home/'.zshrc').write_text('export PATH='+str(home/'.x-cli-demio/bin')+':"$PATH"\n')
    default_env=dict(os.environ,HOME=str(home))
    for key in ('DEMIO_HOME','DEMIO_CONFIG','DEMIO_INSTALL_DIR','XDG_CONFIG_HOME'):
        default_env.pop(key,None)
    default_command=['sh',str(installer),'--archive',str(archive)]
    retired=home/'.x-cli-demio';(retired/'versions').mkdir(parents=True)
    shutil.copytree(install/'bin',retired/'versions/old')
    (retired/'bin').symlink_to(retired/'versions/old')
    (retired/'user-state.txt').write_text('preserve')
    subprocess.run(default_command,check=True,env=default_env)
    assert not (retired/'bin').exists() and not (retired/'versions').exists()
    assert (retired/'user-state.txt').read_text()=='preserve'
    managed=home/'.toolbelt/demio'
    assert 'theme = "forest"' in (managed/'config.toml').read_text()
    assert not (managed/'speaker-notes.json').exists()
    import json
    assert json.loads(subprocess.check_output([str(managed/'bin/demio'),'--overlay-settings'],text=True,env=default_env))=={'font_size':28.0,'alignment':'left'}
    assert 'forest' in subprocess.check_output([str(managed/'bin/demio'),'config'],text=True,env=default_env)
    (managed/'config.toml').write_text('theme = "sunset"\n[overlay]\nfont_size = 26.0\nalignment = "right"\n')
    subprocess.run(default_command,check=True,env=default_env)
    assert 'theme = "sunset"' in (managed/'config.toml').read_text()
    assert json.loads(subprocess.check_output([str(managed/'bin/demio'),'--overlay-settings'],text=True,env=default_env))=={'font_size':26.0,'alignment':'right'}
    assert not (managed/'speaker-notes.json').exists()
    assert (home/'.zshrc').read_text().count(str(managed/'bin'))==1
    assert '.x-cli-demio' not in (home/'.zshrc').read_text()
    # Older machine layouts can link both retired names to the active home.
    # Cleanup must not follow that chain and delete the newly installed bundle.
    alias_home=root/'alias-home';alias_managed=alias_home/'.toolbelt/demio'
    alias_managed.mkdir(parents=True)
    (alias_managed/'config.toml').write_text('theme = "ocean"\n')
    (alias_managed/'demos').mkdir();(alias_managed/'demos/keep.json').write_text('{}')
    (alias_managed/'logs').mkdir();(alias_managed/'logs/keep.log').write_text('keep')
    (alias_home/'.x-product-demio').symlink_to(alias_managed)
    (alias_home/'.x-cli-demio').symlink_to(alias_home/'.x-product-demio')
    alias_env=dict(default_env,HOME=str(alias_home))
    for _ in range(2):
        subprocess.run(default_command+['--no-path-update'],check=True,env=alias_env)
        for executable in ('demio','demio-overlay'):
            assert subprocess.check_output([str(alias_managed/'bin'/executable),'--version'],text=True).strip().split()[-1]==version.split()[-1]
        assert (alias_managed/'config.toml').read_text()=='theme = "ocean"\n'
        assert (alias_managed/'demos/keep.json').read_text()=='{}'
        assert (alias_managed/'logs/keep.log').read_text()=='keep'
        assert (alias_home/'.x-cli-demio').is_symlink() and (alias_home/'.x-product-demio').is_symlink()
    # Exercise update destination inference without fetching a release.
    legacy_bin=home/'.x-cli-demio/bin';shutil.copytree(managed/'bin',legacy_bin)
    stub='#!/bin/sh\nprintf "%s\\n" "$@" > "$(dirname "$0")/update-arguments"\n'
    for folder,expected in [(managed/'bin',["--install-dir",str(managed),"--no-path-update"]),(legacy_bin,["--install-dir",str(managed)])]:
        (folder/'demio-install.sh').write_text(stub)
        subprocess.run([str(folder/'demio'),'update'],check=True,env=default_env)
        assert (folder/'update-arguments').read_text().splitlines()==expected
print('Installer: native versions, checksum rejection, previous installation and config preservation passed.')
