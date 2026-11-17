using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Runtime.Versioning;
using Avalonia.Threading;
using Insert.Core;
using static Insert.Platform.Windows.Native;

namespace Insert.Platform.Windows;

[SupportedOSPlatform("windows")]
internal sealed class WinKeyboard : IKeyboardService, IDisposable
{
    private const int HotkeyId = 1;
    /// <summary>Insert marks the input that it sends, so that its own hooks ignore that input.</summary>
    internal const nuint InputMarker = 0x494E5354;

    private static WinKeyboard? s_current;

    private nint _window;
    private nint _keyboardHook;
    private nint _mouseHook;
    private HoldTrigger? _trigger;
    private readonly HashSet<uint> _downModifiers = [];
    private readonly HashSet<uint> _swallowed = [];

    public WinKeyboard() => s_current = this;

    public event Action? HotkeyPressed;

    public Func<RingKey, bool>? KeyFilter { get; set; }

    public void Attach(nint window) => _window = window;

    public void OnHotkey() => HotkeyPressed?.Invoke();

    public bool RegisterHotkey(Hotkey hotkey)
    {
        UnregisterHotKey(_window, HotkeyId);
        return RegisterHotKey(_window, HotkeyId, (uint)hotkey.Modifiers | MOD_NOREPEAT, (uint)hotkey.Key);
    }

    public unsafe bool StartHoldTrigger(HoldTrigger trigger)
    {
        _trigger = trigger;
        var module = GetModuleHandleW(null);
        _keyboardHook = SetWindowsHookExW(WH_KEYBOARD_LL, &KeyboardProc, module, 0);
        _mouseHook = SetWindowsHookExW(WH_MOUSE_LL, &MouseProc, module, 0);
        trigger.Resync();
        Services.Log.Info($"keyboard: hooks keyboard={_keyboardHook != 0} mouse={_mouseHook != 0}");
        return _keyboardHook != 0 && _mouseHook != 0;
    }

    public IHoldTriggerHost CreateTriggerHost(Func<bool> open, Action release) => new TriggerHost(open, release);

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvStdcall)])]
    private static unsafe nint KeyboardProc(int code, nint wParam, nint lParam)
    {
        var self = s_current;
        if (code >= 0 && self is not null && self.OnKey((int)wParam, (KBDLLHOOKSTRUCT*)lParam)) return 1;
        return CallNextHookEx(0, code, wParam, lParam);
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvStdcall)])]
    private static nint MouseProc(int code, nint wParam, nint lParam)
    {
        if (code >= 0 && s_current?._trigger is { } trigger)
        {
            switch ((int)wParam)
            {
                case WM_LBUTTONDOWN or WM_RBUTTONDOWN or WM_MBUTTONDOWN or WM_XBUTTONDOWN or WM_MOUSEWHEEL or WM_MOUSEHWHEEL:
                    trigger.Interrupted();
                    break;
            }
        }
        return CallNextHookEx(0, code, wParam, lParam);
    }

    /// <summary>Returns true to keep the key from the app in front.</summary>
    private unsafe bool OnKey(int message, KBDLLHOOKSTRUCT* info)
    {
        if (info->dwExtraInfo == InputMarker || _trigger is null) return false;
        var isDown = message is WM_KEYDOWN or WM_SYSKEYDOWN;
        var vk = info->vkCode;

        if (ModifierOf(vk) is not null)
        {
            if (isDown ? _downModifiers.Add(vk) : _downModifiers.Remove(vk))
            {
                _trigger.ModifiersChanged(HeldModifiers());
            }
            return false;
        }

        if (!isDown) return _swallowed.Remove(vk);

        _trigger.Interrupted();
        if (KeyFilter is { } filter && filter(RingKeyOf(vk)))
        {
            _swallowed.Add(vk);
            return true;
        }
        return false;
    }

    private ModifierKeys HeldModifiers()
    {
        var held = ModifierKeys.None;
        foreach (var vk in _downModifiers) held |= ModifierOf(vk) ?? ModifierKeys.None;
        return held;
    }

    private static ModifierKeys? ModifierOf(uint vk) => vk switch
    {
        VK_SHIFT or VK_LSHIFT or VK_RSHIFT => ModifierKeys.Shift,
        VK_CONTROL or VK_LCONTROL or VK_RCONTROL => ModifierKeys.Control,
        VK_MENU or VK_LMENU or VK_RMENU => ModifierKeys.Alt,
        VK_LWIN or VK_RWIN => ModifierKeys.Windows,
        _ => null,
    };

    private static RingKey RingKeyOf(uint vk) => vk switch
    {
        VK_ESCAPE => new RingKey(RingKeyKind.Escape),
        VK_RETURN => new RingKey(RingKeyKind.Enter),
        >= 0x31 and <= 0x39 => new RingKey(RingKeyKind.Digit, (int)vk - 0x30),
        >= 0x61 and <= 0x69 => new RingKey(RingKeyKind.Digit, (int)vk - 0x60),
        _ => new RingKey(RingKeyKind.Other),
    };

    public void SuppressModifierRelease() => Send([Key(VK_DUMMY, up: false), Key(VK_DUMMY, up: true)]);

    public void Restore(WindowHandle window)
    {
        if (window.Value != 0 && IsWindow(window.Value)) SetForegroundWindow(window.Value);
    }

    public async Task Paste(WindowHandle window)
    {
        if (window.Value != 0 && GetForegroundWindow() != window.Value)
        {
            Restore(window);
            await Task.Delay(120);
        }
        else
        {
            await Task.Delay(60);
        }

        // A modifier that the user still holds would change Ctrl+V into another shortcut, for example Ctrl+Shift+V.
        var inputs = new List<INPUT>();
        foreach (var vk in new[] { VK_LSHIFT, VK_RSHIFT, VK_LMENU, VK_RMENU, VK_LWIN, VK_RWIN })
        {
            if (IsDown(vk)) inputs.Add(Key(vk, up: true));
        }
        var controlHeld = IsDown(VK_CONTROL);
        if (!controlHeld) inputs.Add(Key(VK_LCONTROL, up: false));
        inputs.Add(Key(VK_V, up: false));
        inputs.Add(Key(VK_V, up: true));
        if (!controlHeld) inputs.Add(Key(VK_LCONTROL, up: true));
        var sent = Send(inputs.ToArray());
        Services.Log.Info($"paste: sent {sent} of {inputs.Count} key events to window {GetForegroundWindow():X}");
    }

    private static INPUT Key(ushort vk, bool up)
    {
        var extended = vk is VK_RMENU or VK_RCONTROL or VK_LWIN or VK_RWIN;
        return new INPUT
        {
            type = INPUT_KEYBOARD,
            ki = new KEYBDINPUT
            {
                wVk = vk,
                dwFlags = (up ? KEYEVENTF_KEYUP : 0) | (extended ? KEYEVENTF_EXTENDEDKEY : 0),
                dwExtraInfo = InputMarker,
            },
        };
    }

    private static unsafe uint Send(INPUT[] inputs)
    {
        fixed (INPUT* pointer = inputs)
        {
            return SendInput((uint)inputs.Length, pointer, sizeof(INPUT));
        }
    }

    public void Dispose()
    {
        if (_window != 0) UnregisterHotKey(_window, HotkeyId);
        if (_keyboardHook != 0) UnhookWindowsHookEx(_keyboardHook);
        if (_mouseHook != 0) UnhookWindowsHookEx(_mouseHook);
        _keyboardHook = _mouseHook = 0;
    }

    /// <summary>Reads the system state for the hold trigger. The timer runs on the UI thread.</summary>
    private sealed class TriggerHost(Func<bool> open, Action release) : IHoldTriggerHost
    {
        private DispatcherTimer? _timer;

        public HoldTrigger? Trigger { get; set; }

        public void StartTimer(TimeSpan delay)
        {
            _timer?.Stop();
            _timer = new DispatcherTimer(delay, DispatcherPriority.Input, (_, _) =>
            {
                _timer?.Stop();
                if (s_current?._trigger is not { } trigger) return;
                trigger.TimerFired();
                if (trigger.State != HoldState.Holding)
                    Services.Log.Info($"ring: the hold did not open the ring (modifiers {CurrentModifiers}, other key {OtherKeyIsDown}, menu {MenuIsOpen}, pointer {Pointer})");
            });
            _timer.Start();
        }

        public void CancelTimer() => _timer?.Stop();

        public Point2 Pointer => GetCursorPos(out var point) ? new Point2(point.X, point.Y) : default;

        public bool AnyMouseButtonDown =>
            IsDown(VK_LBUTTON) || IsDown(VK_RBUTTON) || IsDown(VK_MBUTTON) || IsDown(VK_XBUTTON1) || IsDown(VK_XBUTTON2);

        public bool OwnWindowIsActive
        {
            get
            {
                GetWindowThreadProcessId(GetForegroundWindow(), out var process);
                return process == (uint)Environment.ProcessId;
            }
        }

        public bool OtherKeyIsDown
        {
            get
            {
                for (var vk = 0x08; vk <= 0xFE; vk++)
                {
                    if (vk == VK_DUMMY || ModifierOf((uint)vk) is not null) continue;
                    if (IsDown(vk)) return true;
                }
                return false;
            }
        }

        public unsafe bool MenuIsOpen
        {
            get
            {
                var foreground = GetForegroundWindow();
                var thread = GetWindowThreadProcessId(foreground, out _);
                var info = new GUITHREADINFO { cbSize = (uint)sizeof(GUITHREADINFO) };
                if (thread != 0 && GetGUIThreadInfo(thread, ref info)
                    && (info.flags & (GUI_INMENUMODE | GUI_POPUPMENUMODE | GUI_SYSTEMMENUMODE)) != 0)
                    return true;

                // "#32768" is the class of a classic Win32 menu window.
                for (var menu = FindWindowExW(0, 0, "#32768", null); menu != 0; menu = FindWindowExW(0, menu, "#32768", null))
                {
                    GetWindowThreadProcessId(menu, out var owner);
                    if (owner != (uint)Environment.ProcessId && IsWindowVisible(menu)) return true;
                }
                return false;
            }
        }

        public ModifierKeys CurrentModifiers =>
            (IsDown(VK_SHIFT) ? ModifierKeys.Shift : 0)
            | (IsDown(VK_CONTROL) ? ModifierKeys.Control : 0)
            | (IsDown(VK_MENU) ? ModifierKeys.Alt : 0)
            | (IsDown(VK_LWIN) || IsDown(VK_RWIN) ? ModifierKeys.Windows : 0);

        public bool Open() => open();

        public void Release() => Dispatcher.UIThread.Post(release);
    }
}
