# Insert

Insert is a minimal native macOS clipboard tray inspired by Paste. It keeps recent text clipboard entries, opens from the bottom of the active screen with a global hotkey, supports search, and includes toggles for hiding the Dock icon and opening at login.

## Download

Download the latest drag-to-install disk image from [GitHub Releases](https://github.com/KenWuqianghao/Insert/releases/latest) (`Insert-Installer.dmg`). Open the DMG and drag **Insert** into **Applications**.

## Build and Run

```sh
make run
```

The app bundle is created at `build/Insert.app`.

To build a DMG locally (releases are built automatically on tagged versions):

```sh
make dmg
```

The DMG is created at `build/Insert-Installer.dmg`.

## Controls

- Global hotkey: `Command+Shift+V`
- Click the keyboard button or use the gear menu to record a custom global hotkey.
- Select a card to copy it back to the clipboard and hide the tray.
- Press Backspace/Delete to remove the selected history item.
- Use the gear menu in the tray to toggle Dock visibility and launch at login.

## Notes

- Clipboard monitoring stores common pasteboard payloads, including text, URLs, files, images, PDFs, rich text, HTML, colors, and typical audio/video image UTIs.
- Text clipboard history is saved across app restarts in `~/Library/Application Support/Insert/ClipboardHistory.json`.
- Launch-at-login uses `SMAppService.mainApp`, which is available on macOS 13 and later and works best after installing the app bundle into `/Applications` with `make install`.
