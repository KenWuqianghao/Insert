Insert for Linux (x86_64)

Insert keeps a history of the clipboard. Press Ctrl+Shift+V to open the tray.

Requirements
- KDE Plasma 6 on Wayland. Other desktops have limits. See the Linux section of
  https://github.com/KenWuqianghao/Insert
- GTK 4.14 or later and glibc 2.39 or later.

Install
  ./install.sh
The script copies the program to ~/.local/bin/insert. It adds the application
menu entry and the icon in ~/.local/share. Root access is not necessary.

Start
Start Insert from the application menu, or run ~/.local/bin/insert.
Insert opens no window at the start. Plasma asks you to confirm the shortcut.

Remove
  ./install.sh --uninstall
Your clips stay in ~/.local/share/insert. Your settings stay in ~/.config/insert.
