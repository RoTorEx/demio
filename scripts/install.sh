#!/bin/sh
set -eu
# All writes are staged until archive entries and both native versions pass.
install_root=${DEMIO_INSTALL_DIR:-${DEMIO_HOME:-"$HOME/.toolbelt/demio"}}
archive=
version=
path_update=1
while [ "$#" -gt 0 ]; do
  case "$1" in
    --install-dir) install_root=$2; shift 2 ;;
    --archive) archive=$2; shift 2 ;;
    --version) version=$2; shift 2 ;;
    --no-path-update) path_update=0; shift ;;
    *) echo "Unknown option: $1" >&2; exit 2 ;;
  esac
done
[ "$(uname -s)" = Darwin ] && [ "$(uname -m)" = arm64 ] || { echo 'Demio 0.1 supports macOS Apple Silicon.' >&2; exit 1; }
command -v python3 >/dev/null || { echo 'Python 3 is required to verify the bundle.' >&2; exit 1; }
install_root=$(python3 -c 'import os,sys;print(os.path.abspath(os.path.expanduser(sys.argv[1])))' "$install_root")
stage=$(mktemp -d "${TMPDIR:-/tmp}/demio-install.XXXXXXXX")
trap 'rm -rf "$stage"' EXIT HUP INT TERM
if [ -z "$archive" ]; then
  if [ -z "$version" ]; then
    latest_url=$(curl -fsSL -o /dev/null -w '%{url_effective}' https://github.com/RoTorEx/demio/releases/latest)
    version=${latest_url##*/v}
  fi
  case "$version" in *[!0-9.]*|'') echo 'Invalid release version' >&2; exit 1 ;; esac
  file="demio-v${version}-aarch64-apple-darwin.tar.gz"
  base="https://github.com/RoTorEx/demio/releases/download/v${version}"
  curl -fsSL "$base/$file" -o "$stage/$file"
  curl -fsSL "$base/$file.sha256" -o "$stage/$file.sha256"
  archive="$stage/$file"
fi
mkdir "$stage/bundle"
python3 - "$archive" "$stage/bundle" "$version" <<'PY'
import hashlib, pathlib, subprocess, sys, tarfile
source, destination, requested = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2]), sys.argv[3]
expected = pathlib.Path(str(source)+'.sha256').read_text().split()[0]
if hashlib.sha256(source.read_bytes()).hexdigest() != expected:
    raise SystemExit('Archive checksum mismatch')
with tarfile.open(source) as bundle:
    entries = bundle.getmembers()
    if len(entries) != 5 or {m.name for m in entries} != {'demio','demio-overlay','demio-install.sh','VERSION','LICENSE'} or any(not m.isfile() for m in entries):
        raise SystemExit('Unexpected archive contents')
    for entry in entries:
        (destination/entry.name).write_bytes(bundle.extractfile(entry).read())
version = (destination/'VERSION').read_text().strip()
if requested and version != requested:
    raise SystemExit('Requested version does not match bundle')
for program in ('demio', 'demio-overlay'):
    binary = destination/program
    binary.chmod(0o755)
    if subprocess.check_output([str(binary),'--version'], text=True).strip() != f'{program} {version}':
        raise SystemExit('Native version mismatch')
PY
# Keep one current bundle. A failed swap restores the previous bin directory.
# Data directories are outside bin and are never replaced by an update.
installed_version=$(cat "$stage/bundle/VERSION")
python3 - "$stage/bundle" "$install_root" <<'PYINSTALL'
import os, pathlib, shutil, subprocess, sys, tempfile
source, root = map(pathlib.Path, sys.argv[1:])
root.mkdir(parents=True, exist_ok=True)
with tempfile.TemporaryDirectory(prefix='.install-', dir=root) as temporary:
    temporary = pathlib.Path(temporary)
    candidate, previous = temporary/'bin', temporary/'previous'
    shutil.copytree(source, candidate)
    current = root/'bin'
    had_current = current.exists() or current.is_symlink()
    if had_current:
        os.replace(current, previous)
    try:
        os.replace(candidate, current)
    except BaseException:
        if had_current:
            os.replace(previous, current)
        raise
# Old Demio installers stored only application bundles here. Remove that
# legacy layout after the new bundle is in place, never user data directories.
versions = root/'versions'
if versions.is_dir() and not versions.is_symlink():
    bundles = list(versions.iterdir())
    allowed = {'demio', 'demio-overlay', 'demio-install.sh', 'VERSION', 'LICENSE'}
    if all(p.is_dir() and not p.is_symlink() and {c.name for c in p.iterdir()} == allowed for p in bundles):
        shutil.rmtree(versions)
(root/'demos').mkdir(exist_ok=True)
(root/'logs').mkdir(exist_ok=True)
# Import legacy preferences once, preserving the originals and custom configs.
if root == pathlib.Path.home()/'.toolbelt/demio':
    legacy = pathlib.Path(os.environ.get('XDG_CONFIG_HOME', pathlib.Path.home()/'.config'))/'demio'
    for name in ('config.toml', 'speaker-notes.json'):
        if not (root/name).exists() and (legacy/name).is_file():
            shutil.copy2(legacy/name, root/name)
if not (root/'config.toml').exists():
    (root/'config.toml').write_text('theme = "calm"\n\n[channels]\n')
# The CLI owns TOML parsing and the one-time speaker preferences migration.
subprocess.run([str(root/'bin/demio'), '--overlay-settings'],
               env=dict(os.environ, DEMIO_CONFIG=str(root/'config.toml')),
               stdout=subprocess.DEVNULL, check=True)
# The old default home is retired only after the replacement works. Preserve
# any user state there; delete only recognizable executable bundles and links.
if root == pathlib.Path.home()/'.toolbelt/demio':
    legacy_root = pathlib.Path.home()/'.x-cli-demio'
    legacy_versions = legacy_root/'versions'
    allowed = {'demio', 'demio-overlay', 'demio-install.sh', 'VERSION', 'LICENSE'}
    def owned_bundle(path):
        return (path.is_dir() and not path.is_symlink()
                and {p.name for p in path.iterdir()} == allowed
                and all(p.is_file() and not p.is_symlink() for p in path.iterdir()))
    bundles = list(legacy_versions.iterdir()) if legacy_versions.is_dir() and not legacy_versions.is_symlink() else []
    if legacy_root.resolve() != root.resolve() and all(owned_bundle(path) for path in bundles):
        legacy_bin = legacy_root/'bin'
        if legacy_bin.is_symlink() and legacy_bin.resolve() in bundles:
            legacy_bin.unlink()
        elif owned_bundle(legacy_bin):
            shutil.rmtree(legacy_bin)
        if legacy_versions.is_dir() and not legacy_versions.is_symlink():
            shutil.rmtree(legacy_versions)
        try:
            legacy_root.rmdir()
        except OSError:
            pass
PYINSTALL
if [ "$path_update" = 1 ]; then
  python3 - "$install_root/bin" "$HOME/.zshrc" <<'PY'
import pathlib, shlex,sys
profile=pathlib.Path(sys.argv[2]); line='export PATH='+shlex.quote(sys.argv[1])+':"$PATH"'
text=profile.read_text() if profile.exists() else ''
legacy='export PATH='+shlex.quote(str(pathlib.Path.home()/'.x-cli-demio/bin'))+':"$PATH"'
text=text.replace(legacy,line)
profile.write_text(text)
if line not in text.splitlines():
    with profile.open('a') as stream: stream.write('\n# Demio CLI\n'+line+'\n')
PY
fi
printf 'Installed Demio %s at %s/bin/demio\n' "$installed_version" "$install_root"
printf 'Open a new terminal, then run: demio help\n'
