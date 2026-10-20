#!/usr/bin/env python3
from datetime import datetime
from pathlib import Path
import re
import subprocess
from zoneinfo import ZoneInfo
root = Path(__file__).resolve().parents[1]
def run(*args):
    return subprocess.check_output(args, cwd=root, text=True).strip()
if run('git','status','--porcelain'):
    raise SystemExit('Commit normal work first; release requires a clean worktree.')
if run('git','branch','--show-current') != 'main':
    raise SystemExit('Release from main.')
version = input('Release version (MAJOR.MINOR.PATCH): ').strip()
if not re.fullmatch(r'(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)', version):
    raise SystemExit('Invalid SemVer')
old = re.search(r'^version = "([^"]+)"', (root/'demio-cli/Cargo.toml').read_text(), re.M).group(1)
if tuple(map(int,version.split('.'))) <= tuple(map(int,old.split('.'))):
    raise SystemExit('Version must increase')
if run('git','tag','--list','v'+version):
    raise SystemExit('Tag exists')
subprocess.run(['make','check'], cwd=root, check=True)
p=root/'demio-cli/Cargo.toml';p.write_text(p.read_text().replace(f'version = "{old}"',f'version = "{version}"',1))
p=root/'demio-overlay/Overlay.swift';p.write_text(p.read_text().replace(f'let buildVersion = "{old}"',f'let buildVersion = "{version}"'))
subprocess.run(['cargo','update','--workspace','--offline'],cwd=root,check=True)
p=root/'CHANGELOG.md'; text=p.read_text(); marker='## [Unreleased]'; date=datetime.now(ZoneInfo('Europe/Warsaw')).date();p.write_text(text.replace(marker,f'{marker}\n\n## [{version}] - {date}',1))
subprocess.run(['git','add','demio-cli/Cargo.toml','Cargo.lock','demio-overlay/Overlay.swift','CHANGELOG.md'],cwd=root,check=True)
subprocess.run(['git','commit','-m',f'chore(release): release {version}'],cwd=root,check=True)
subprocess.run(['git','tag','-a','v'+version,'-m','Demio '+version],cwd=root,check=True)
print('Created annotated v'+version+'. Publish with make release-push.')
