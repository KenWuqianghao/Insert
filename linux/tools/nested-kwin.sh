#!/usr/bin/env bash
# Runs a script inside a private D-Bus session and a virtual 1280x800 KWin.
# Keys and windows stay out of the real desktop. WAYLAND_DISPLAY is set before the D-Bus session starts,
# so services that D-Bus activates (portals) also open their windows in the virtual KWin.
# Example: linux/tools/nested-kwin.sh ./my-test.sh
set -euo pipefail
export XDG_CURRENT_DESKTOP=KDE XDG_SESSION_TYPE=wayland KDE_FULL_SESSION=true
export WAYLAND_DISPLAY=insert-test
unset DISPLAY
exec dbus-run-session -- kwin_wayland --virtual --width 1280 --height 800 \
  --socket insert-test --xwayland --exit-with-session "$(realpath "$1")"
