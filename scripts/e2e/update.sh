#!/usr/bin/env bash
# Checks self-updating end to end: builds this version's AppImage and a
# 99.0.0 one, both pointing at a local update server, then updates the
# first to the second through the command palette in a container.
#
#   TAURI_SIGNING_PRIVATE_KEY=<key or path> \
#   TAURI_SIGNING_PRIVATE_KEY_PASSWORD=<password> scripts/e2e/update.sh
#
# The key must match the public key in src-tauri/tauri.conf.json.
set -euo pipefail
: "${TAURI_SIGNING_PRIVATE_KEY:?set it to the updater signing key}"

root=$(cd "$(dirname "$0")/../.." && pwd)
work="$root/target/e2e/update"
rm -rf "$work"
mkdir -p "$work/serve"
cd "$root"

build() { # <version|""> <output name>
  local version=${1:+\"version\": \"$1\",}
  cat > "$work/conf.json" <<JSON
{
  $version
  "bundle": { "targets": ["appimage"], "createUpdaterArtifacts": true },
  "plugins": {
    "updater": {
      "endpoints": ["http://127.0.0.1:8766/latest.json"],
      "dangerousInsecureTransportProtocol": true
    }
  }
}
JSON
  npx tauri build --config "$work/conf.json" >/dev/null
  local image
  image=$(ls -t target/release/bundle/appimage/*.AppImage | head -1)
  cp "$image" "$work/serve/$2.AppImage"
  cp "$image.sig" "$work/serve/$2.AppImage.sig"
}

echo "== building the current version"
build "" old
echo "== building 99.0.0"
build 99.0.0 new
cat > "$work/serve/latest.json" <<JSON
{
  "version": "99.0.0",
  "notes": "Test update",
  "pub_date": "2026-01-01T00:00:00Z",
  "platforms": {
    "linux-x86_64-appimage": {
      "signature": "$(cat "$work/serve/new.AppImage.sig")",
      "url": "http://127.0.0.1:8766/new.AppImage"
    }
  }
}
JSON

docker run --rm -v "$work:/work" -v "$root/scripts/e2e:/e2e:ro" fedora:44 bash /e2e/update-inside.sh
