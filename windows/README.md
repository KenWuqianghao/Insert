# Insert for Windows

Insert is a clipboard tray. Press the shortcut and the tray comes up from the bottom of the screen. Select a clip and Insert pastes it into the app that you use.

This is the Windows port of [Insert for macOS](https://github.com/KenWuqianghao/Insert). It is a C# and Avalonia app with the same tray, search, pinboards, preview, clip ring, and settings. Insert has no account and no cloud sync. All data stays on your computer.

## Requirements

- Windows 10 version 1903 or later, or Windows 11. The app is for x64 or Arm64.
- No .NET installation is necessary. `Insert.exe` contains the runtime.

## Install

1. Download `Insert-<version>-win-x64.zip` (or `win-arm64`) from the releases.
2. Extract `Insert.exe` to a folder that you keep, for example `%LOCALAPPDATA%\Programs\Insert`.
3. Start `Insert.exe`. Insert opens no window. Its icon appears in the notification area.
4. Copy some text, then press `Ctrl+Shift+V`. The tray opens.

To start Insert when you log in, turn on **Open Insert at login** in Settings. If you move `Insert.exe` after that, turn the setting off and on again.

## Controls

| Action | Shortcut |
| --- | --- |
| Open or close Insert | `Ctrl+Shift+V` |
| Move the selection | Arrow keys |
| Extend the selection | `Shift` + arrow keys |
| Paste the selected clip | `Enter` |
| Paste as plain text | `Shift+Enter` |
| Paste card 1 to 9 | `Ctrl+1` to `Ctrl+9` |
| Show the numbers of the cards | Hold `Ctrl` |
| Copy the selected clip | `Ctrl+C` |
| Select all clips | `Ctrl+A` |
| Delete the selected clips | `Delete` |
| Preview | `Space` |
| Go to the next or the previous pinboard | `Tab` or `Shift+Tab` |
| Search | Type in the tray |
| Close | `Esc` |
| Open Settings | `Ctrl+,` |

Right-click a clip to paste it, pin it to a pinboard, rename it, or delete it. Right-click a pinboard to rename it, change its color, or delete it.

### Clip ring

1. Hold `Alt` alone for 0.3 seconds. Do not move the pointer. A ring of your 8 most recent clips opens at the pointer.
2. Move the pointer onto a clip. A preview of the clip shows next to the ring.
3. Release `Alt`. Insert pastes that clip into the app in front.

If you release `Alt` while the pointer is in the center of the ring, the ring stays open. Then click a clip, or its preview, to paste it. Press `Esc` or click outside the ring to close it. `1` to `8` paste a clip by its position.

To use `Ctrl` or the Windows key in place of `Alt`, change **Hold to open the clip ring** in Settings. The ring does not open while the menu of another app is open, or when you press another key or a mouse button during the hold.

## Command line

`Insert.exe` accepts these options. When Insert already runs, the new process sends the command to it and stops. With no option, a second start opens the tray.

| Option | Result |
| --- | --- |
| `--show` | Open the tray. |
| `--toggle` | Open or close the tray. |
| `--settings` | Open Settings. |
| `--quit` | Stop Insert. |
| `--background` | Start with no window. If Insert already runs, do nothing. Launch at login uses this option. |
| `--data <folder>` | Keep the data in `<folder>`. Each folder has its own Insert process. |

## Build from source

You need the .NET 10 SDK. The commands work on Windows, macOS, and Linux.

```sh
dotnet build windows/src/Insert
dotnet test windows/tests/Insert.Core.Tests
```

To make the release archives, run `windows/tools/publish.sh`. It writes `Insert-<version>-win-x64.zip` and `Insert-<version>-win-arm64.zip` to `windows/dist/`. Each archive contains one self-contained `Insert.exe`.

### Run the app on Linux or macOS

The Win32 parts are behind small interfaces in `src/Insert/Platform`. On another system, Insert uses fake services: the clipboard is in memory, and a paste is a line in the log. Use this to work on the UI:

```sh
dotnet run --project windows/src/Insert -- --data /tmp/insert-demo --demo
dotnet run --project windows/src/Insert -- --data /tmp/insert-demo --show
```

`--demo` adds sample clips to an empty library. These options exist only with the fake services:

| Option | Result |
| --- | --- |
| `--ring [n]` | Open the clip ring, and select segment `n`. |
| `--copy <text>` | Put the text on the fake clipboard. Insert saves it as a clip. |
| `--pointer x,y` | Move the fake pointer, in screen pixels. |

## Storage

```text
%LOCALAPPDATA%\Insert\
├── index.json          # clip metadata and pinboards
├── preferences.json    # settings
├── payloads\           # one file for each clip, with all its clipboard formats
├── thumbnails\         # one PNG for each image clip
└── insert.log          # the log, 1 MB maximum
```

Launch at login adds the value `Insert` to `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run`.

Insert does not save a clip that is larger than 50 MB. Insert does not save a clip that an app marks as private, for example a password from a password manager.

Version 2 does not read the history of the WinForms version. The old file `%APPDATA%\Insert\ClipboardHistory.json` stays on disk. Delete it if you do not need it.

## Known limits

- Insert cannot paste into an app that runs as administrator, and the clip ring does not open over it. Windows blocks input from a normal app to an elevated app.
- The tray uses acrylic glass when Windows allows it. If transparency effects are off in the Windows settings, the tray is solid. The clip ring has no blur.
- The icon of a Microsoft Store app can show as a generic icon.
- The macOS setting **Hide Dock icon** is **Show the icon in the notification area** on Windows.
- There is no automatic update. Download the new version and replace `Insert.exe`.

## Source

The source is in [`windows/`](https://github.com/KenWuqianghao/Insert/tree/main/windows) in the main Insert repository. Open issues and pull requests in the main repository.

The icons in `src/Insert/Assets/Icons.axaml` come from [Fluent UI System Icons](https://github.com/microsoft/fluentui-system-icons), copyright Microsoft Corporation, under the MIT license.
