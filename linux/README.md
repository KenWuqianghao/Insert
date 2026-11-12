# Insert for Linux

Insert is a clipboard tray. Press the shortcut and the tray comes up from the bottom of the screen. Select a clip and Insert pastes it into the app that you use.

This is the Linux port of [Insert for macOS](https://github.com/KenWuqianghao/Insert). It is a Rust and GTK 4 app with the same tray, search, pinboards, preview, and settings. Insert has no account and no cloud sync. All data stays on your computer.

Product page: https://insert-app.vercel.app

## Requirements

- KDE Plasma 6 on Wayland. This is the only desktop that is tested.
- GTK 4.14 or later and glibc 2.39 or later.

On GNOME, Insert cannot watch the clipboard, because GNOME does not have the data-control protocol. The tray shows a notice. X11 sessions are not tested.

## Install

1. Download `insert-linux-x86_64.tar.gz` from the [latest release](https://github.com/KenWuqianghao/insert-linux/releases/latest).
2. Extract the archive and run `./install.sh`. Root access is not necessary.
3. Start **Insert** from the application menu. Insert opens no window. Its icon appears in the system tray.
4. Plasma asks for the global shortcut. Click **OK** to accept `Ctrl+Shift+V`.
5. Copy some text, then press `Ctrl+Shift+V`. The tray opens.
6. Click **Allow…** in the tray notice. Plasma asks to let Insert control input devices. Click **Approve**. After this, `Return` pastes the selected clip into the app in front.

To remove Insert, run `./install.sh --uninstall`. Your clips and your settings stay.

## Controls

| Action | Shortcut |
| --- | --- |
| Open or close Insert | `Ctrl+Shift+V` |
| Move the selection | Arrow keys |
| Paste the selected clip | `Return` |
| Paste as plain text | `Shift+Return` |
| Paste card 1 to 9 | `Ctrl+1` to `Ctrl+9` |
| Preview | `Space` |
| Go to the next pinboard | `Tab` |
| Search | Type in the tray |
| Close | `Esc` |
| Open Settings | `Ctrl+,` |

If your desktop has no global shortcut service, bind `~/.local/bin/insert --toggle` to a key in the desktop settings.

## Build from source

The build runs in a podman container, so the host needs no compiler.

```sh
CARGO_BUILD_JOBS=2 ./build.sh build --release
./install.sh
```

`./package.sh` makes the release archive in `dist/`.

## Storage

Insert keeps its clips in `~/.local/share/insert/` and its settings in `~/.config/insert/preferences.json`. Only your user can read these files.

## Source

The source is in [`linux/`](https://github.com/KenWuqianghao/Insert/tree/main/linux) in the main Insert repository. The [insert-linux](https://github.com/KenWuqianghao/insert-linux) repository is a mirror of that folder and has the releases. Open issues and pull requests in the main repository.
