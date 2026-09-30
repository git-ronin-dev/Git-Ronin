#!/usr/bin/env bash
# Runs inside the container started by smoke.sh.
set -euo pipefail

echo "== installing $BUNDLE"
if command -v dnf >/dev/null; then
  # An AppImage leaves the graphics libraries (EGL, GL) to the system.
  pkgs="xorg-x11-server-Xvfb xdotool ImageMagick git dbus-x11 python3 procps-ng google-noto-sans-fonts
    mesa-libEGL mesa-libGL mesa-dri-drivers libglvnd-gles"
  [ "$SESSION" = wayland ] && pkgs="$pkgs sway grim libcap"
  case "$BUNDLE" in
    *.rpm) dnf install -y -q "$BUNDLE" $pkgs >/dev/null ;;
    *) dnf install -y -q $pkgs >/dev/null ;;
  esac
else
  export DEBIAN_FRONTEND=noninteractive
  apt-get update -qq
  pkgs="xvfb xdotool imagemagick git dbus-x11 python3 procps fonts-noto-core libegl1 libgl1
    libgles2 libgl1-mesa-dri"
  [ "$SESSION" = wayland ] && pkgs="$pkgs sway grim"
  case "$BUNDLE" in
    *.deb) apt-get install -y -qq "$BUNDLE" $pkgs >/dev/null ;;
    *) apt-get install -y -qq $pkgs >/dev/null ;;
  esac
fi
case "$BUNDLE" in
  *.flatpak)
    dnf install -y -q flatpak >/dev/null
    flatpak remote-add --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
    flatpak install -y --noninteractive flathub org.gnome.Platform//50 >/dev/null
    flatpak install -y --noninteractive --bundle "$BUNDLE" >/dev/null
    app=(flatpak run com.gitronin.desktop)
    ;;
  *.AppImage)
    cp "$BUNDLE" /usr/local/bin/git-ronin.AppImage
    chmod +x /usr/local/bin/git-ronin.AppImage
    # No FUSE in a container.
    app=(env APPIMAGE_EXTRACT_AND_RUN=1 /usr/local/bin/git-ronin.AppImage)
    ;;
  *) app=(git-ronin) ;;
esac
id tester >/dev/null 2>&1 || useradd -m tester
[ -s /etc/machine-id ] || dbus-uuidgen > /etc/machine-id

# A server that accepts connections and never answers keeps git's fetch
# (git-remote-http) running long enough to look at its environment.
python3 - <<'PY' &
import socket
s = socket.socket()
s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
s.bind(("127.0.0.1", 8765))
s.listen()
held = []
while True:
    held.append(s.accept()[0])
PY

su tester -c 'bash -s' <<'SH'
set -e
cd ~
git init -q repo
git -C repo -c user.name=T -c user.email=t@e commit -q --allow-empty -m first
git -C repo remote add origin http://127.0.0.1:8765/repo.git
# The second is where a Flatpak keeps its settings.
for cfg in ~/.config/com.gitronin.desktop ~/.var/app/com.gitronin.desktop/config/com.gitronin.desktop; do
  mkdir -p "$cfg"
  printf 'openTabs = ["%s"]\nactiveTab = "%s"\n' ~/repo ~/repo > "$cfg/local.toml"
  printf '[git]\nautoFetchMinutes = 1\n' > "$cfg/portable.toml"
done
SH

# An X11 server, or a headless Wayland compositor.
if [ "$SESSION" = wayland ]; then
  run=/tmp/xdg-run
  install -d -o tester -m 700 "$run"
  : > /tmp/sway.conf
  # Fedora gives sway file capabilities, which a container refuses to exec.
  setcap -r "$(command -v sway)" 2>/dev/null || true
  su tester -c "XDG_RUNTIME_DIR=$run WLR_BACKENDS=headless WLR_RENDERER=pixman \
    WLR_LIBINPUT_NO_DEVICES=1 sway -c /tmp/sway.conf > /tmp/sway.log 2>&1 &"
  for _ in $(seq 20); do
    ls $run/sway-ipc.*.sock >/dev/null 2>&1 && break
    sleep 0.5
  done
  ls $run/sway-ipc.*.sock >/dev/null 2>&1 || { cat /tmp/sway.log; exit 1; }
  session="XDG_RUNTIME_DIR=$run WAYLAND_DISPLAY=wayland-1 GDK_BACKEND=wayland"
  as_session() { su tester -c "$session SWAYSOCK=\$(ls $run/sway-ipc.*.sock) $*"; }
  has_window() { as_session swaymsg -t get_tree | grep -q '"name": "Git Ronin"'; }
  # Through /tmp: tester may not own the mounted output folder.
  shoot() { as_session grim /tmp/shot.png && cp /tmp/shot.png "$1"; }
  # Shortcuts match physical keys (KeyboardEvent.code), which a virtual
  # keyboard such as wtype's doesn't reproduce.
  settings() { return 1; }
else
  Xvfb :99 -screen 0 1400x900x24 >/dev/null 2>&1 &
  sleep 1
  session="DISPLAY=:99"
  has_window() { DISPLAY=:99 xdotool search --name '^Git Ronin$' >/dev/null 2>&1; }
  shoot() { DISPLAY=:99 import -window root "$1"; }
  settings() {
    # No window manager: focus rather than activate.
    DISPLAY=:99 xdotool windowfocus --sync "$(DISPLAY=:99 xdotool search --name '^Git Ronin$' | head -1)"
    DISPLAY=:99 xdotool key ctrl+comma
  }
fi
echo "== starting ${app[*]} ($SESSION)"
su tester -c "$session dbus-launch ${app[*]} > /tmp/app.log 2>&1 &"

fail() {
  echo "FAIL: $*"
  shoot "/out/$SHOT" 2>/dev/null && echo "screenshot: target/e2e/$SHOT"
  cat /tmp/app.log
  exit 1
}

for _ in $(seq 60); do
  has_window && break
  sleep 1
done
has_window || fail "no window"
echo "== window open"

# Auto-fetch starts about 5 s after the repository opens.
remote=""
for _ in $(seq 40); do
  remote=$(pgrep -f 'git-remote-http' | head -1 || true)
  [ -n "$remote" ] && break
  sleep 1
done
[ -n "$remote" ] || fail "git never fetched"
# Readable by its own user only (root lacks ptrace rights in a container).
environ=$(su tester -c "cat /proc/$remote/environ" | tr '\0' '\n')
[ -n "$environ" ] || fail "could not read git's environment"
# The AppImage's library folders, or the Flatpak's own config folder (where
# git would miss ~/.config/git).
leaked=$(echo "$environ" |
  grep -E '^(APPDIR|APPIMAGE|GTK_PATH|GIO_MODULE_DIR)=|^LD_LIBRARY_PATH=.*(mount_|appimage)|^XDG_CONFIG_HOME=.*/\.var/app/' || true)
[ -z "$leaked" ] || fail "git inherited the bundle's environment: $leaked"
echo "== git runs with a clean environment"

sleep 3
shoot "/out/$SHOT"
echo "== screenshot: target/e2e/$SHOT"
# Settings → General names the kind of install and whether it updates.
if settings; then
  sleep 2
  shoot "/out/${SHOT%.png}-settings.png"
  echo "== screenshot: target/e2e/${SHOT%.png}-settings.png"
fi
grep -iE "panic|error" /tmp/app.log && echo "(app log above)" || true
echo PASS
