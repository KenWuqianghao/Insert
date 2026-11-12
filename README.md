# Insert

![The Insert tray at the bottom of the screen, with a row of clip cards](docs/assets/insert-tray.png)

**Insert** is a native clipboard tray for macOS. Press the shortcut and the tray comes up from the bottom of the screen. Select a clip and Insert pastes it into the app that you use.

Product page: https://insert-app.vercel.app

Insert has no account and no cloud sync. All data stays on your Mac.

## Demo

[![Demo of Insert: open the tray, preview a clip, search, change the pinboard, and paste](docs/assets/insert-demo.gif)](docs/assets/insert-demo.mp4)

Click the demo to open the [full-quality video](docs/assets/insert-demo.mp4). The demo shows these steps:

1. Press `Command+Shift+V` to open the tray.
2. Press the arrow keys to move the selection. Press `Space` to see a large preview.
3. Type to search.
4. Press `Tab` to go to the next pinboard.
5. Hold `Command`, then press `1` to paste the first card into the email.

## Highlights

- A full-width tray at the bottom of the screen, inspired by Paste.
- Cards show the source app, the type, the time, and a preview of the content.
- Press `Enter` to paste the selected clip into the active app.
- Press `Shift+Enter` to paste the clip as plain text.
- Type to search. Use the filter to show one type of clip.
- Pinboards keep clips that you use frequently. The history limit does not delete pinned clips.
- Press `Space` to see a large preview of a clip.
- Hold `Command` to show the numbers 1 to 9 on the cards. Press `Command+1` to `Command+9` to paste that card.
- Insert skips items that an app marks as concealed or transient, for example passwords from a password manager.
- You can pause capture, and you can ignore the apps that you select.
- The Settings window has two tabs. **General** has launch at login, the Dock icon, the global shortcut, direct paste, and the history limit. **Privacy** has the ignored apps, the capture pause, and the command to clear the history.
- Insert stores text, links, files, images, PDFs, rich text, colors, and common media types.

![The large preview of an image clip, above the row of cards](docs/assets/insert-preview.png)

<img src="docs/assets/insert-settings.png" alt="The General tab of the Settings window" width="500">

## Download

Download the latest drag-to-install disk image from [GitHub Releases](https://github.com/KenWuqianghao/Insert/releases/latest) and open `Insert-Installer.dmg`. Drag **Insert** into **Applications**.

Local builds are written to:

```sh
build/Insert-Installer.dmg
```

## First Open

Insert is not notarized by Apple, so macOS blocks it the first time that you open it. Open **System Settings** > **Privacy & Security** and click **Open Anyway**. Updates install through Sparkle and do not show this prompt again.

## Release

`Tools/release.sh` publishes a new version:

```sh
Tools/release.sh 1.2.0 --dry-run
Tools/release.sh 1.2.0
```

The script does these steps:

1. Sets `CFBundleShortVersionString` to the version and increases `CFBundleVersion` by 1 in `Info.plist`.
2. Runs `make dmg`.
3. Copies the DMG to `Insert-Update.dmg` and signs the copy with Sparkle's `sign_update`.
4. Creates the GitHub release `v1.2.0` on this repository with `Insert-Installer.dmg` and `Insert-Update.dmg`. The two files are the same. The appcast points to `Insert-Update.dmg`, so the download count of each file is separate for new installs and for updates.
5. Writes the appcast to `$APPCAST`. The default is `../insert-site/appcast.xml`.

`--dry-run` does steps 1 to 3, prints the appcast, and puts `Info.plist` back. It does not create the release.

After a release, commit `Info.plist` here, then commit and deploy `appcast.xml` in insert-site.

The Sparkle signing key is in the login Keychain, in the item `https://sparkle-project.org` (account `ed25519`). Back up this item. If you lose it, the installed copies of Insert cannot accept updates. To export a backup file, run `vendor/Sparkle-2.10.1/bin/generate_keys -x <file>`. Keep the file in a safe place. The first time `sign_update` runs, the Keychain asks for permission. Click **Always Allow**.

To see the download counts of each release, run `Tools/downloads.sh`. It reads the counts of this repository and of the Windows port from GitHub.

`make notarize` notarizes and staples the DMG. It needs a Developer ID certificate and a `notarytool` profile, so it is for later. Run it before Sparkle signs the DMG.

## Controls

| Action | Shortcut |
| --- | --- |
| Open or close Insert | Global shortcut, default `Command+Shift+V` |
| Move the selection | Arrow keys |
| Extend the selection | `Shift` + arrow keys |
| Select all clips | `Command+A` |
| Paste the selected clip | `Enter` or double-click |
| Paste as plain text | `Shift+Enter` |
| Copy without paste | `Command+C` |
| Paste card 1 to 9 | `Command+1` to `Command+9` |
| Preview | `Space` (when the search field is empty) |
| Delete the selection | `Backspace` or `Delete` (when the search field is empty), or `Command+Backspace` |
| Go to the next or previous pinboard | `Tab` / `Shift+Tab`, or `Command+]` / `Command+[` |
| Search | Type in the tray |
| Close the preview, clear the search, or close the tray | `Esc` |
| Open Settings | `Command+,` |

Right-click a card to pin, rename, preview, or delete the clip. Right-click a pinboard tab to rename it, change its color, or delete it.

## Accessibility Permission

Insert pastes with a simulated `Command+V`. macOS permits this only for apps that have the Accessibility permission.

1. Open Insert. If the permission is missing, the tray shows a notice.
2. Click **Allow…** in the notice.
3. Turn on **Insert** in System Settings > Privacy & Security > Accessibility.

Without the permission, Insert copies the clip and you press `Command+V`. To use copy only, turn off **Paste directly into the active app** in Settings.

A local build has an ad hoc signature. macOS can ask for the permission again after each new build.

## Build From Source

```sh
make run
```

The app bundle is created at `build/Insert.app`.

To build a second copy with its own bundle id, storage folder, and settings:

```sh
make dev
```

This creates `build/InsertDev.app`. Use it to test a build while an installed Insert runs.

To make the screenshots and the demo video in this README again:

```sh
make marketing-assets
```

This command records the real tray views with sample clips. It does not read your clipboard. It needs `ffmpeg` and the Screen Recording permission for your terminal. Keep the screen unlocked while it runs. If the recording loses frames, the command records the demo again. It makes a maximum of three takes.

To build the installer:

```sh
make dmg
```

## Windows

The Windows-specific repo is available at:

https://github.com/KenWuqianghao/insert-windows

The mirrored source under [`windows/Insert.Windows.sln`](./windows/Insert.Windows.sln) and [`windows/Insert.Windows`](./windows/Insert.Windows) is a native WinForms port with the same clipboard tray, search, hotkey, and persistent history model.

Build it on Windows with the .NET 8 Windows Desktop workload installed:

```sh
dotnet build windows/Insert.Windows.sln
```

## Linux

The Linux port is in [`linux/`](./linux). It is a Rust and GTK 4 app with the same tray, search, pinboards, and history model.

### Requirements

- KDE Plasma 6 on Wayland. This is the only desktop that is tested.
- GTK 4.14 or later and glibc 2.39 or later, for the release build.
- `podman`, to build from source. The build runs in a container, so the host needs no compiler.

GNOME on Wayland has these limits:

- Insert cannot watch the clipboard, because GNOME does not have the data-control protocol. The tray shows a notice, and Insert saves no clips.
- Insert cannot ask for the keyboard focus. If the tray opens without the focus, click the tray.
- Insert does not know which app is in front. Clips have no source app, and the ignored apps list does not work.

X11 sessions are not tested.

### Build and install

```sh
CARGO_BUILD_JOBS=2 linux/build.sh build --release
linux/install.sh
```

`install.sh` copies the app to `~/.local/bin/insert`. It adds the application menu entry and the icon in `~/.local/share`. Root access is not necessary. To remove these files, run `linux/install.sh --uninstall`. Your clips and your settings stay.

To make a release archive, run `linux/package.sh`. The archive is `linux/dist/insert-linux-x86_64.tar.gz`. It contains the app, the menu entry, the icon, and `install.sh`.

### First run

1. Start **Insert** from the application menu, or run `~/.local/bin/insert`. Insert opens no window. Its icon appears in the system tray.
2. Plasma shows the dialog **Global Shortcuts Requested**. Click **OK** to accept `Ctrl+Shift+V`.
3. Copy some text. Then press `Ctrl+Shift+V`. The tray opens.
4. Click **Allow…** in the tray notice. Plasma shows a dialog with the line **Control input devices**. Click **Approve**.

After step 4, `Return` pastes the selected clip into the app in front. Without this permission, Insert copies the clip and you press `Ctrl+V`.

Insert pastes with a simulated `Ctrl+V`. In a terminal, Insert sends `Shift+Insert`, and it also puts the clip in the primary selection.

The tray keys are the keys in [Controls](#controls), with `Ctrl` in place of `Command`.

To change the shortcut, open Settings and click **Change…**. To start Insert when you log in, turn on **Open Insert at login** in Settings.

### The `insert --toggle` command

`insert --toggle` opens or closes the tray of the app that runs. If your desktop has no global shortcut service, bind this command to a key in the desktop settings:

```sh
~/.local/bin/insert --toggle
```

### Storage on Linux

```sh
~/.local/share/insert/
├── index.json        # clip metadata and pinboards
├── payloads/         # one folder for each clip, with one file for each data type
└── thumbnails/       # one PNG for each image clip
~/.config/insert/preferences.json                              # settings
~/.config/autostart/io.github.kenwuqianghao.Insert.desktop     # launch at login
~/.local/state/insert/remote-desktop-token                     # the paste permission
```

If `XDG_DATA_HOME`, `XDG_CONFIG_HOME`, or `XDG_STATE_HOME` is set, Insert uses that folder.

### Known limits on Linux

- The tray is a full-screen transparent window, because a Wayland app cannot set its position. A click above the tray closes it.
- Insert finds a terminal by its app id. The list is in [`paste.rs`](./linux/crates/insert-platform/src/paste.rs). Direct paste into a terminal that is not in the list does not work.
- If you remove the paste permission in System Settings, click **Allow…** in the tray again.
- The macOS setting **Hide Dock icon** is **Show the icon in the system tray** on Linux.
- There is no automatic update. Install a new build with `install.sh`.

## Storage

Insert keeps its data in a folder that has the name of the bundle id:

```sh
~/Library/Application Support/com.local.Insert/
├── index.json        # clip metadata and pinboards
├── payloads/         # one file for each clip, with the full pasteboard content
└── thumbnails/       # one PNG for each image clip
```

Insert does not store an item that is larger than 50 MB.

Version 0.2 does not read the history from version 0.1. The old file `~/Library/Application Support/Insert/ClipboardHistory.json` stays on disk. Delete it if you do not need it.

Windows startup is stored in the current user's `Run` registry key and is toggled from the tray menu.
