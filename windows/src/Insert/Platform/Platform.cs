using Avalonia;
using Avalonia.Controls;
using Insert.Core;

namespace Insert.Platform;

/// <summary>Pixels of an icon: straight (not premultiplied) BGRA, top row first.</summary>
public sealed record IconPixels(byte[] Bgra, int Width, int Height);

/// <summary>The system services that Insert uses. <c>WindowsPlatform</c> uses Win32. <c>FakePlatform</c> runs the UI on other systems.</summary>
public interface IPlatform : IDisposable
{
    /// <summary>A short name for the log and the About text, for example "Windows".</summary>
    string Name { get; }

    /// <summary>Starts the services that need the UI thread. Call it once, after Avalonia starts.</summary>
    void Start();

    IClipboardService Clipboard { get; }
    IAppService Apps { get; }
    IKeyboardService Keyboard { get; }
    IWindowService Windows { get; }
    ILoginItem LoginItem { get; }
}

public interface IClipboardService
{
    /// <summary>Another app changed the clipboard. Raised on the UI thread.</summary>
    event Action? Changed;

    /// <summary>Reads the formats that Insert stores. Returns null for a private copy, or when no stored format is present.</summary>
    ClipPayload? Read();

    /// <summary>Writes a clip. The change that this causes does not raise <see cref="Changed"/>.</summary>
    bool Write(ClipPayload payload);
}

/// <summary>A window of another app, to give back the keyboard before a paste. Zero means none.</summary>
public readonly record struct WindowHandle(nint Value)
{
    public static readonly WindowHandle None = new(0);
}

public interface IAppService
{
    /// <summary>The app that has the window in front.</summary>
    SourceApp? ForegroundApp();

    WindowHandle ForegroundWindow();

    /// <summary>The app of an executable file, for the list of ignored apps.</summary>
    SourceApp? Describe(string executablePath);

    IconPixels? AppIcon(SourceApp app);

    IconPixels? FileIcon(string path);
}

public interface IKeyboardService
{
    /// <summary>The user pressed the global shortcut. Raised on the UI thread.</summary>
    event Action? HotkeyPressed;

    /// <summary>Returns false when another app uses the shortcut.</summary>
    bool RegisterHotkey(Hotkey hotkey);

    /// <summary>Feeds the keyboard and mouse hooks to the trigger. Returns false when the hooks are not available.</summary>
    bool StartHoldTrigger(HoldTrigger trigger);

    /// <summary>The host side of the trigger: the system state that the trigger reads.</summary>
    IHoldTriggerHost CreateTriggerHost(Func<bool> open, Action release);

    /// <summary>
    /// While set, key presses for other apps go to this function first. It returns true to keep the key from the app.
    /// The ring uses it, because the ring does not take the keyboard.
    /// </summary>
    Func<RingKey, bool>? KeyFilter { get; set; }

    /// <summary>
    /// Sends a key with no function. Windows opens the menu bar when the user releases Alt alone, and Start when the user releases
    /// the Windows key alone. A key press between the press and the release stops this.
    /// </summary>
    void SuppressModifierRelease();

    /// <summary>Gives the keyboard back to <paramref name="window"/>, if set, then sends Ctrl+V.</summary>
    Task Paste(WindowHandle window);

    /// <summary>Gives the keyboard back to <paramref name="window"/> without a paste.</summary>
    void Restore(WindowHandle window);
}

/// <summary>A key press that the ring reads.</summary>
public readonly record struct RingKey(RingKeyKind Kind, int Digit = 0);

public enum RingKeyKind
{
    Other,
    Escape,
    Enter,
    Digit,
}

public interface IWindowService
{
    /// <summary>The pointer position in physical pixels.</summary>
    PixelPoint Pointer { get; }

    void MovePointer(PixelPoint position);

    /// <summary>The window does not take the keyboard when the user clicks it.</summary>
    void MakeNonActivating(Window window);

    /// <summary>Sets the position and the size of a window in one step, in physical pixels.</summary>
    void Place(Window window, PixelRect frame);

    /// <summary>Makes the window the foreground window, so that it gets the keyboard.</summary>
    void Activate(Window window);
}

public interface ILoginItem
{
    bool IsEnabled { get; }

    /// <summary>Returns an error message, or null when the change worked.</summary>
    string? SetEnabled(bool enabled);
}
