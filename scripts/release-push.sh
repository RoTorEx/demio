#!/bin/sh
set -eu
version=$(sed -n 's/^version = "\(.*\)"/\1/p' demio-cli/Cargo.toml)
tag="v$version"
[ -z "$(git status --porcelain)" ] || { echo 'Worktree must be clean' >&2; exit 1; }
[ "$(git branch --show-current)" = main ] || exit 1
git rev-parse --verify '@{upstream}' >/dev/null
[ "$(git rev-parse "$tag^{commit}")" = "$(git rev-parse HEAD)" ] || { echo 'Tag must point to HEAD' >&2; exit 1; }
[ "$(git cat-file -t "$tag")" = tag ] || { echo 'Annotated tag required' >&2; exit 1; }
git push
git push origin "$tag"
