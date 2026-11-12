#!/usr/bin/env bash
#   install.sh              install or update
#   install.sh --uninstall  remove the program files; the clips and the settings stay
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
app_id=io.github.kenwuqianghao.Insert
bin_dir="$HOME/.local/bin"
data_home="${XDG_DATA_HOME:-$HOME/.local/share}"
config_home="${XDG_CONFIG_HOME:-$HOME/.config}"
desktop_file="$data_home/applications/$app_id.desktop"
icon_sizes=(256x256 512x512)

refresh_desktop_database() {
  if command -v update-desktop-database >/dev/null; then
    update-desktop-database "$data_home/applications" || true
  fi
}

case "${1:-}" in
  "")
    binary="$here/insert"
    [ -f "$binary" ] || binary="$here/target/release/insert"
    if [ ! -f "$binary" ]; then
      echo "install.sh: no binary. Run linux/build.sh build --release first." >&2
      exit 1
    fi
    install -Dm755 "$binary" "$bin_dir/insert"
    install -Dm644 "$here/data/$app_id.desktop" "$desktop_file"
    # The desktop starts the app with its own PATH, which can be without ~/.local/bin.
    sed -i "s|^Exec=.*|Exec=\"$bin_dir/insert\"|" "$desktop_file"
    for size in "${icon_sizes[@]}"; do
      install -Dm644 "$here/data/icons/hicolor/$size/apps/$app_id.png" \
        "$data_home/icons/hicolor/$size/apps/$app_id.png"
    done
    refresh_desktop_database
    echo "Insert is installed. Start it from the application menu, or run $bin_dir/insert"
    ;;
  --uninstall)
    rm -f "$bin_dir/insert" "$desktop_file" "$config_home/autostart/$app_id.desktop"
    for size in "${icon_sizes[@]}"; do
      rm -f "$data_home/icons/hicolor/$size/apps/$app_id.png"
    done
    refresh_desktop_database
    echo "Insert is removed. Your clips are in $data_home/insert and your settings are in $config_home/insert."
    ;;
  *)
    echo "usage: install.sh [--uninstall]" >&2
    exit 2
    ;;
esac
