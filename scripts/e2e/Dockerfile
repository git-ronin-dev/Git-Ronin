# Image for driving a debug build under Xvfb (see plan.md, "End-to-end
# checks"): docker build -t git-ronin-e2e scripts/e2e
FROM fedora:44
RUN dnf install -y -q webkit2gtk4.1 libappindicator-gtk3 librsvg2 xorg-x11-server-Xvfb xdotool \
      ImageMagick git dbus-x11 google-noto-sans-fonts google-noto-sans-mono-fonts >/dev/null \
    && dnf clean all
RUN useradd -u 1000 -m tester && dbus-uuidgen > /etc/machine-id
