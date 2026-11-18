using System.Runtime.InteropServices;
using System.Runtime.Versioning;
using Avalonia;
using Avalonia.Controls;
using Microsoft.Win32;
using static Insert.Platform.Windows.Native;

namespace Insert.Platform.Windows;

[SupportedOSPlatform("windows")]
public sealed unsafe class WindowsPlatform : IPlatform, IWindowService, ILoginItem
{
    private static WindowsPlatform? s_current;
    private nint _messageWindow;

    public WindowsPlatform()
    {
        s_current = this;
        ClipboardService = new WinClipboard();
        KeyboardService = new WinKeyboard();
        AppService = new WinApps();
    }

    public string Name => "Windows";
    public IClipboardService Clipboard => ClipboardService;
    public IAppService Apps => AppService;
    public IKeyboardService Keyboard => KeyboardService;
    public IWindowService Windows => this;
    public ILoginItem LoginItem => this;

    internal WinClipboard ClipboardService { get; }
    internal WinKeyboard KeyboardService { get; }
    internal WinApps AppService { get; }

    /// <summary>A message-only window gets WM_CLIPBOARDUPDATE and WM_HOTKEY on the UI thread.</summary>
    public void Start()
    {
        const string className = "Insert.MessageWindow";
        fixed (char* name = className)
        {
            var windowClass = new WNDCLASSEXW
            {
                cbSize = (uint)sizeof(WNDCLASSEXW),
                lpfnWndProc = &WindowProc,
                hInstance = GetModuleHandleW(null),
                lpszClassName = name,
            };
            RegisterClassExW(&windowClass);
        }
        _messageWindow = CreateWindowExW(0, className, "Insert", 0, 0, 0, 0, 0, HWND_MESSAGE, 0, GetModuleHandleW(null), 0);
        ClipboardService.Attach(_messageWindow);
        KeyboardService.Attach(_messageWindow);
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(System.Runtime.CompilerServices.CallConvStdcall)])]
    private static nint WindowProc(nint hwnd, uint message, nint wParam, nint lParam)
    {
        var platform = s_current;
        try
        {
            switch (message)
            {
                case WM_CLIPBOARDUPDATE when platform is not null:
                    platform.ClipboardService.OnClipboardUpdate();
                    return 0;
                case WM_HOTKEY when platform is not null:
                    platform.KeyboardService.OnHotkey();
                    return 0;
            }
        }
        catch (Exception error)
        {
            // An exception that leaves an UnmanagedCallersOnly method ends the process.
            Services.Log.Info($"window: message {message:X} error {error}");
        }
        return DefWindowProcW(hwnd, message, wParam, lParam);
    }

    public PixelPoint Pointer => GetCursorPos(out var point) ? new PixelPoint(point.X, point.Y) : default;

    public void MovePointer(PixelPoint position) => SetCursorPos(position.X, position.Y);

    public void MakeNonActivating(Window window)
    {
        if (Handle(window) is not { } hwnd) return;
        var style = (long)GetWindowLongPtr(hwnd, GWL_EXSTYLE);
        SetWindowLongPtr(hwnd, GWL_EXSTYLE, (nint)(style | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW));
    }

    public void Place(Window window, PixelRect frame)
    {
        if (Handle(window) is not { } hwnd)
        {
            window.Position = frame.Position;
            return;
        }
        SetWindowPos(hwnd, HWND_TOPMOST, frame.X, frame.Y, frame.Width, frame.Height, SWP_NOACTIVATE);
    }

    public void Activate(Window window)
    {
        window.Activate();
        if (Handle(window) is { } hwnd) SetForegroundWindow(hwnd);
    }

    private static nint? Handle(Window window) => window.TryGetPlatformHandle()?.Handle is { } handle && handle != 0 ? handle : null;

    private const string RunKey = @"Software\Microsoft\Windows\CurrentVersion\Run";
    private const string RunValue = "Insert";

    private static string LaunchCommand => $"\"{Environment.ProcessPath}\" --background";

    public bool IsEnabled
    {
        get
        {
            using var key = Registry.CurrentUser.OpenSubKey(RunKey);
            return key?.GetValue(RunValue) is string command && command.Contains(Environment.ProcessPath ?? "\0", StringComparison.OrdinalIgnoreCase);
        }
    }

    public string? SetEnabled(bool enabled)
    {
        try
        {
            using var key = Registry.CurrentUser.CreateSubKey(RunKey, writable: true);
            if (enabled)
                key.SetValue(RunValue, LaunchCommand);
            else
                key.DeleteValue(RunValue, throwOnMissingValue: false);
            return null;
        }
        catch (Exception error) when (error is UnauthorizedAccessException or System.Security.SecurityException or IOException)
        {
            return error.Message;
        }
    }

    public void Dispose()
    {
        KeyboardService.Dispose();
        ClipboardService.Dispose();
        if (_messageWindow != 0) DestroyWindow(_messageWindow);
        _messageWindow = 0;
    }
}
