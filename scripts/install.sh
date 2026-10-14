#!/bin/sh
set -eu
# All writes are staged until archive entries and both native versions pass.
install_root=${DEMIO_INSTALL_DIR:-"$HOME/.x-cli-demio"}
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
    version=$(curl -fsSL https://api.github.com/repos/RoTorEx/demio/releases/latest | python3 -c 'import json,sys;print(json.load(sys.stdin)["tag_name"].removeprefix("v"))')
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
# Version directories plus a single pointer swap keep CLI and helper together.
mkdir -p "$install_root/versions"
installed_version=$(cat "$stage/bundle/VERSION")
release_dir=$(mktemp -d "$install_root/versions/$installed_version.XXXXXXXX")
cp "$stage/bundle/"* "$release_dir/"
ln -s "$release_dir" "$install_root/.bin-next-$$"
if [ -e "$install_root/bin" ] && [ ! -L "$install_root/bin" ]; then
  rm "$install_root/.bin-next-$$"
  echo 'Existing bin is not a symlink; choose another --install-dir.' >&2
  exit 1
fi
python3 - "$install_root/.bin-next-$$" "$install_root/bin" <<'PY'
import os,sys
os.replace(sys.argv[1],sys.argv[2])
PY
if [ "$path_update" = 1 ]; then
  python3 - "$install_root/bin" "$HOME/.zshrc" <<'PY'
import pathlib, shlex,sys
profile=pathlib.Path(sys.argv[2]); line='export PATH='+shlex.quote(sys.argv[1])+':"$PATH"'
text=profile.read_text() if profile.exists() else ''
if line not in text.splitlines():
    with profile.open('a') as stream: stream.write('\n# Demio CLI\n'+line+'\n')
PY
fi
printf 'Installed Demio %s at %s/bin/demio\n' "$installed_version" "$install_root"
printf 'Open a new terminal, then run: demio help\n'
