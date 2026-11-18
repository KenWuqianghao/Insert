using Avalonia;
using Avalonia.Controls;
using Avalonia.Media.Imaging;
using Avalonia.Platform;
using Avalonia.Threading;
using Insert.Core;
using Insert.Services;

namespace Insert.Platform.Fake;

/// <summary>
/// Runs the full UI on a system without Win32, for tests and screenshots. Nothing leaves the process:
/// the clipboard is a variable, a paste writes a log line, and the global shortcut and the ring trigger come from the command channel.
/// </summary>
public sealed class FakePlatform : IPlatform, IClipboardService, IAppService, IKeyboardService, IWindowService, ILoginItem
{
    private ClipPayload? _clipboard;
    private bool _loginItem;

    public string Name => "Fake";
    public IClipboardService Clipboard => this;
    public IAppService Apps => this;
    public IKeyboardService Keyboard => this;
    public IWindowService Windows => this;
    public ILoginItem LoginItem => this;

    public void Start()
    {
    }

    public event Action? Changed;
    public event Action? HotkeyPressed;

    /// <summary>Puts text on the fake clipboard as if another app copied it.</summary>
    public void SimulateCopy(ClipPayload payload)
    {
        _clipboard = payload;
        Changed?.Invoke();
    }

    public void SimulateHotkey() => HotkeyPressed?.Invoke();

    public ClipPayload? Read() => _clipboard;

    public bool Write(ClipPayload payload)
    {
        _clipboard = payload;
        Log.Info($"fake clipboard: wrote {string.Join(", ", payload.Representations.Select(r => r.Format))}");
        return true;
    }

    public SourceApp? ForegroundApp() => null;

    public WindowHandle ForegroundWindow() => WindowHandle.None;

    public SourceApp? Describe(string executablePath) => new(executablePath, Path.GetFileNameWithoutExtension(executablePath));

    /// <summary>A demo app id is the path of a PNG icon.</summary>
    public IconPixels? AppIcon(SourceApp app) => LoadPng(app.Id);

    public IconPixels? FileIcon(string path) => null;

    private static IconPixels? LoadPng(string path)
    {
        if (!path.EndsWith(".png", StringComparison.OrdinalIgnoreCase) || !File.Exists(path)) return null;
        using var bitmap = new Bitmap(path);
        var size = bitmap.PixelSize;
        var pixels = new byte[size.Width * size.Height * 4];
        using (var context = new RenderTargetBitmap(size))
        {
            using (var drawing = context.CreateDrawingContext())
                drawing.DrawImage(bitmap, new Rect(0, 0, size.Width, size.Height));
            unsafe
            {
                fixed (byte* data = pixels)
                    context.CopyPixels(new PixelRect(size), (nint)data, pixels.Length, size.Width * 4);
            }
        }
        // RenderTargetBitmap gives premultiplied pixels.
        for (var index = 0; index < pixels.Length; index += 4)
        {
            var alpha = pixels[index + 3];
            if (alpha is 0 or 255) continue;
            for (var channel = 0; channel < 3; channel++)
                pixels[index + channel] = (byte)Math.Min(255, pixels[index + channel] * 255 / alpha);
        }
        return new IconPixels(pixels, size.Width, size.Height);
    }

    public bool RegisterHotkey(Hotkey hotkey)
    {
        Log.Info($"fake keyboard: shortcut {hotkey.DisplayName}");
        return true;
    }

    public bool StartHoldTrigger(HoldTrigger trigger) => false;

    public IHoldTriggerHost CreateTriggerHost(Func<bool> open, Action release) => new Host(open, release);

    public Func<RingKey, bool>? KeyFilter { get; set; }

    public void SuppressModifierRelease() => Log.Info("fake keyboard: dummy key");

    public Task Paste(WindowHandle window)
    {
        Log.Info("fake keyboard: Ctrl+V");
        return Task.CompletedTask;
    }

    public void Restore(WindowHandle window)
    {
    }

    /// <summary>The app sets it to the center of the primary screen. A test command can move it.</summary>
    public PixelPoint Pointer { get; set; }

    public void MovePointer(PixelPoint position) => Pointer = position;

    public void MakeNonActivating(Window window)
    {
    }

    public void Place(Window window, PixelRect frame)
    {
        var scaling = window.Screens.ScreenFromPoint(frame.Center)?.Scaling ?? 1;
        window.Position = frame.Position;
        window.Width = frame.Width / scaling;
        window.Height = frame.Height / scaling;
    }

    public void Activate(Window window) => window.Activate();

    public bool IsEnabled => _loginItem;

    public string? SetEnabled(bool enabled)
    {
        _loginItem = enabled;
        return null;
    }

    public void Dispose()
    {
    }

    private sealed class Host(Func<bool> open, Action release) : IHoldTriggerHost
    {
        public void StartTimer(TimeSpan delay)
        {
        }

        public void CancelTimer()
        {
        }

        public Point2 Pointer => default;
        public bool AnyMouseButtonDown => false;
        public bool OwnWindowIsActive => false;
        public bool OtherKeyIsDown => false;
        public bool MenuIsOpen => false;
        public ModifierKeys CurrentModifiers => ModifierKeys.None;
        public bool Open() => open();
        public void Release() => Dispatcher.UIThread.Post(release);
    }
}
