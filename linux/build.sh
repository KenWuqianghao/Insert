#!/usr/bin/env bash
# Example: linux/build.sh build --release
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
image=insert-linux-build
if ! podman image exists "$image"; then
  podman build -t "$image" -f "$here/Containerfile" "$here"
fi
podman volume exists insert-cargo-registry || podman volume create insert-cargo-registry >/dev/null
exec podman run --rm \
  -v "$here":/src:z \
  -v insert-cargo-registry:/opt/cargo/registry \
  -e CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-3}" \
  -w /src "$image" cargo "$@"
