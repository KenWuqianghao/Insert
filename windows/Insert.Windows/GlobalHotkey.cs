using System;
using System.Runtime.InteropServices;
using System.Windows.Forms;

namespace Insert.Windows;

[Flags]
enum HotkeyModifiers : uint
{
    None = 0,
    Alt = 0x0001,
    Control = 0x0002,
    Shift = 0x0004,
    Win = 0x0008
}

sealed class GlobalHotkey : IDisposable
{
    private const int WM_HOTKEY = 0x0312;
    private const int HOTKEY_ID = 0xB17C;

    private readonly NotifyWindow _window;
    private readonly Action _action;

    public GlobalHotkey(Action action, Keys key, HotkeyModifiers modifiers)
    {
        _action = action;
        _window = new NotifyWindow(this);
        Register(modifiers, key);
    }

    private void Register(HotkeyModifiers modifiers, Keys key)
    {
        if (!RegisterHotKey(_window.Handle, HOTKEY_ID, (uint)modifiers, (uint)key))
        {
            throw new InvalidOperationException("Unable to register global hotkey.");
        }
    }

    public void Dispose()
    {
        UnregisterHotKey(_window.Handle, HOTKEY_ID);
        _window.Dispose();
    }

    private void Trigger() => _action();

    private sealed class NotifyWindow : NativeWindow, IDisposable
    {
        private readonly GlobalHotkey _owner;

        public NotifyWindow(GlobalHotkey owner)
        {
            _owner = owner;
            CreateHandle(new CreateParams());
        }

        protected override void WndProc(ref Message m)
        {
            if (m.Msg == WM_HOTKEY)
            {
                _owner.Trigger();
                return;
            }

            base.WndProc(ref m);
        }

        public void Dispose()
        {
            DestroyHandle();
        }
    }

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool RegisterHotKey(IntPtr hWnd, int id, uint fsModifiers, uint vk);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool UnregisterHotKey(IntPtr hWnd, int id);
}

