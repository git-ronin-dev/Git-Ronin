#!/usr/bin/env bash
# Installs a release bundle in a clean container and checks that it starts:
# the window opens on a repository, and the git it runs (a fetch from a
# server that never answers) doesn't inherit the bundle's libraries.
#
#   scripts/e2e/smoke.sh [--wayland] <bundle> [image]
#
# <bundle> is a .rpm, .deb or .AppImage (e.g. from target/release/bundle), or
# a .flatpak (`flatpak build-bundle`). The image defaults to fedora:44, or
# ubuntu:24.04 for a .deb. A screenshot lands in target/e2e/. Flatpak runs
# need a privileged container and keep the runtime in the `ronin-flatpak`
# volume. --wayland runs it in a headless sway instead of Xvfb.
set -euo pipefail

session=x11
if [ "${1:-}" = --wayland ]; then
  session=wayland
  shift
fi

bundle=$(realpath "$1")
name=$(basename "$bundle")
case "$name" in
  *.deb) image=${2:-ubuntu:24.04} ;;
  *.rpm | *.AppImage | *.flatpak) image=${2:-fedora:44} ;;
  *) echo "not a bundle: $name" >&2; exit 2 ;;
esac
root=$(cd "$(dirname "$0")/../.." && pwd)
mkdir -p "$root/target/e2e"
shot="$(echo "$name-$image-$session" | tr -c 'A-Za-z0-9._\n-' '_').png"

extra=()
case "$name" in
  *.flatpak) extra=(--privileged -v ronin-flatpak:/var/lib/flatpak) ;;
esac

docker run --rm "${extra[@]}" \
  -v "$bundle:/bundle/$name:ro" \
  -v "$root/scripts/e2e:/e2e:ro" \
  -v "$root/target/e2e:/out" \
  -e BUNDLE="/bundle/$name" -e SHOT="$shot" -e SESSION="$session" \
  "$image" bash /e2e/inside.sh
