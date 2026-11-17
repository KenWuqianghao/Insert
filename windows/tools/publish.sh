#!/usr/bin/env bash
# Makes dist/Insert-<version>-<rid>.zip: one self-contained Insert.exe for each runtime.
# Usage: tools/publish.sh [rid ...]   (default: win-x64 win-arm64)
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
version="$(sed -n 's:.*<Version>\(.*\)</Version>.*:\1:p' "$root/Directory.Build.props")"
rids=("$@")
[ ${#rids[@]} -gt 0 ] || rids=(win-x64 win-arm64)

mkdir -p "$root/dist"
for rid in "${rids[@]}"; do
  out="$root/dist/$rid"
  rm -rf "$out"
  dotnet publish "$root/src/Insert/Insert.csproj" -c Release -r "$rid" -o "$out" -m:2 \
    -p:SelfContained=true \
    -p:PublishSingleFile=true \
    -p:IncludeNativeLibrariesForSelfExtract=true \
    -p:EnableCompressionInSingleFile=true \
    -p:DebugType=none
  rm -f "$out"/*.pdb
  zip="$root/dist/Insert-$version-$rid.zip"
  rm -f "$zip"
  (cd "$out" && zip -q -9 "$zip" Insert.exe)
  echo "$zip"
done
