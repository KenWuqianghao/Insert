#!/usr/bin/env bash
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
name=insert-linux-x86_64
CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}" "$here/build.sh" build --release -p insert
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/$name" "$here/dist"
cp "$here/target/release/insert" "$here/install.sh" "$stage/$name/"
cp "$here/data/README.txt" "$stage/$name/"
cp -r "$here/data" "$stage/$name/data"
rm "$stage/$name/data/README.txt"
tar -C "$stage" --owner=0 --group=0 -czf "$here/dist/$name.tar.gz" "$name"
echo "$here/dist/$name.tar.gz"
