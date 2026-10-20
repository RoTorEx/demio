#!/usr/bin/env python3
import hashlib
import os
from pathlib import Path
import tarfile
import re
root = Path(__file__).resolve().parents[1]
version = re.search(r'^version = "([^"]+)"', (root/'demio-cli/Cargo.toml').read_text(), re.M).group(1)
target = Path(os.environ.get('CARGO_TARGET_DIR', str(Path.home()/'construction_side/demio/target')))
out = target.parent/'dist'
out.mkdir(parents=True, exist_ok=True)
name = f'demio-v{version}-aarch64-apple-darwin.tar.gz'
archive = out/name
version_file = out/'VERSION'
version_file.write_text(version+'\n')
with tarfile.open(archive, 'w:gz') as bundle:
    for source, destination in [(target/'release/demio','demio'),(target/'release/demio-overlay','demio-overlay'),(root/'scripts/install.sh','demio-install.sh'),(root/'LICENSE','LICENSE'),(version_file,'VERSION')]:
        bundle.add(source, arcname=destination, recursive=False)
(archive.with_name(name+'.sha256')).write_text(hashlib.sha256(archive.read_bytes()).hexdigest()+'  '+name+'\n')
(out/'install.sh').write_bytes((root/'scripts/install.sh').read_bytes())
print(archive)
