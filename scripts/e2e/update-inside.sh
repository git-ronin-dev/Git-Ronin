#!/usr/bin/env bash
# Runs inside the container started by update.sh.
set -euo pipefail

dnf install -y -q xorg-x11-server-Xvfb xdotool ImageMagick git dbus-x11 python3 procps-ng \
  google-noto-sans-fonts mesa-libEGL mesa-libGL mesa-dri-drivers libglvnd-gles >/dev/null
useradd -m tester
dbus-uuidgen > /etc/machine-id
install -o tester -m 755 /work/serve/old.AppImage /home/tester/git-ronin.AppImage
(cd /work/serve && python3 -m http.server 8766 --bind 127.0.0.1 >/dev/null 2>&1) &

Xvfb :99 -screen 0 1400x900x24 >/dev/null 2>&1 &
export DISPLAY=:99
sleep 1
start() {
  su tester -c "cd ~ && DISPLAY=:99 APPIMAGE_EXTRACT_AND_RUN=1 dbus-launch ./git-ronin.AppImage >> /tmp/app.log 2>&1 &"
}
window() {
  for _ in $(seq 60); do
    xdotool search --name '^Git Ronin$' 2>/dev/null | head -1 && return
    sleep 1
  done
  return 1
}
fail() {
  echo "FAIL: $*"
  import -window root /work/fail.png || true
  cat /tmp/app.log
  exit 1
}

start
win=$(window) || fail "no window"
sleep 3
old=$(md5sum < /home/tester/git-ronin.AppImage)
echo "== running the old version; checking for updates from the palette"
xdotool windowfocus --sync "$win"
xdotool key ctrl+p
sleep 1
xdotool type --delay 30 "Check for updates"
sleep 1
xdotool key Return
# The confirmation dialog, with the release notes.
sleep 4
import -window root /work/confirm.png
xdotool key Return
echo "== installing"
for _ in $(seq 60); do
  [ "$(md5sum < /home/tester/git-ronin.AppImage)" != "$old" ] && break
  sleep 1
done
[ "$(md5sum < /home/tester/git-ronin.AppImage)" = "$(md5sum < /work/serve/new.AppImage)" ] ||
  fail "the AppImage was not replaced"
echo "== AppImage replaced; waiting for the restart"
sleep 8
win=$(window) || fail "no window after the restart"
xdotool windowfocus --sync "$win"
xdotool key ctrl+comma
sleep 2
import -window root /work/after.png
echo "== screenshots in target/e2e/update/"
echo PASS
