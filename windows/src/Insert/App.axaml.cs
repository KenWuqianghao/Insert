using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.ApplicationLifetimes;
using Avalonia.Markup.Xaml;
using Avalonia.Platform;
using Avalonia.Threading;
using Insert.Core;
using Insert.Platform;
using Insert.Platform.Fake;
using Insert.Services;
using Insert.Views;

namespace Insert;

/// <summary>Builds the services and the windows, and connects them.</summary>
public sealed partial class App : Application
{
    public static Options Options { get; set; } = new(ClipStorage.DefaultRoot, "start", false);
    public static CommandChannel? Channel { get; set; }

    private IClassicDesktopStyleApplicationLifetime? _desktop;
    private IPlatform _platform = null!;
    private Preferences _preferences = null!;
    private ClipLibrary _library = null!;
    private TrayWindow _tray = null!;
    private RingWindow _ring = null!;
    private SettingsWindow? _settings;
    private SettingsViewModel _settingsModel = null!;
    private HoldTrigger? _trigger;
    private TrayIcon? _trayIcon;
    private NativeMenuItem? _pauseItem;
    private DispatcherTimer _saveTimer = null!;
    private bool _hotkeyIsUnavailable;

    public override void Initialize() => AvaloniaXamlLoader.Load(this);

    public override void OnFrameworkInitializationCompleted()
    {
        if (ApplicationLifetime is IClassicDesktopStyleApplicationLifetime desktop)
        {
            _desktop = desktop;
            Start();
            desktop.Exit += (_, _) => Stop();
        }
        base.OnFrameworkInitializationCompleted();
    }

    private void Start()
    {
        var root = Options.DataRoot;
        Log.Start(root);
        _platform = OperatingSystem.IsWindows() ? new Platform.Windows.WindowsPlatform() : new FakePlatform();
        _platform.Start();
        Log.Info($"start: Insert {typeof(App).Assembly.GetName().Version} on {_platform.Name}, data in {root}");

        _preferences = new Preferences(new PreferencesStore(Path.Combine(root, "preferences.json")));
        _library = new ClipLibrary(new ClipStorage(root), _preferences.Data.HistoryLimit);
        var images = new ImageCodec();
        var icons = new IconStore(_platform.Apps);
        if (Options.Demo) DemoData.Seed(_library, images, DemoApp);

        new ClipboardMonitor(_platform, _library, _preferences, images).Start();
        var paste = new PasteService(_platform, _library, _preferences);

        var model = new PanelModel(_library, (delay, action) => DispatcherTimer.RunOnce(action, delay));
        var trayModel = new TrayViewModel(model, icons, _preferences);
        _tray = new TrayWindow(trayModel, _platform, paste)
        {
            ShowSettings = ShowSettings,
            ClearHistory = ConfirmClearHistory,
            Quit = Quit,
            SetCapturePaused = paused => _preferences.Update(data => data with { CapturePaused = paused }),
        };
        model.ShowSettings = ShowSettings;

        _ring = new RingWindow(_library, icons, _platform, paste, trayModel.Thumbnail);
        _settingsModel = new SettingsViewModel(_preferences, _platform, icons);

        if (_platform is FakePlatform fake)
        {
            var screen = _tray.Screens.Primary;
            fake.Pointer = screen?.WorkingArea.Center ?? new PixelPoint(640, 400);
        }

        var host = _platform.Keyboard.CreateTriggerHost(() => _ring.ShowRing(), () => _ring.EndHold());
        _trigger = new HoldTrigger(host) { Modifier = _preferences.Data.RingModifier };
        _settingsModel.RingIsUnavailable = !_platform.Keyboard.StartHoldTrigger(_trigger) && _platform is not FakePlatform;

        _platform.Keyboard.HotkeyPressed += () => _tray.Toggle();
        RegisterHotkey();

        _saveTimer = new DispatcherTimer(TimeSpan.FromMilliseconds(250), DispatcherPriority.Background, (_, _) =>
        {
            _saveTimer.Stop();
            Save();
        });
        _library.Changed += () =>
        {
            _saveTimer.Stop();
            _saveTimer.Start();
        };

        _preferences.Changed += OnPreferencesChanged;
        CreateTrayIcon();
        Channel?.Listen(HandleCommand);
        if (Options.Command is not ("start" or "background")) HandleCommand(Options.Command);
    }

    private SourceApp DemoApp(string name)
    {
        var icon = new[] { 64, 48 }.Select(size => $"/usr/share/icons/hicolor/{size}x{size}/apps/{name}.png").FirstOrDefault(File.Exists);
        var title = char.ToUpperInvariant(name[0]) + name[1..];
        return new SourceApp(icon ?? $@"C:\Program Files\{title}\{name}.exe", title);
    }

    private void OnPreferencesChanged(PreferencesData data)
    {
        _library.HistoryLimit = data.HistoryLimit;
        if (_trigger is not null) _trigger.Modifier = data.RingModifier;
        if (_trayIcon is not null) _trayIcon.IsVisible = data.ShowTrayIcon;
        if (_pauseItem is not null) _pauseItem.Header = data.CapturePaused ? "Resume Capture" : "Pause Capture";
        if (_trayIcon is not null) _trayIcon.ToolTipText = data.CapturePaused ? "Insert, capture is paused" : "Insert";
        RegisterHotkey();
    }

    private Hotkey? _registered;
    /// <summary>The last shortcut that registered. It stays in use when a new shortcut is used by another app.</summary>
    private Hotkey? _working;

    private void RegisterHotkey()
    {
        var hotkey = _preferences.Data.Hotkey;
        if (hotkey == _registered) return;
        _registered = hotkey;
        _hotkeyIsUnavailable = !_platform.Keyboard.RegisterHotkey(hotkey);
        if (!_hotkeyIsUnavailable) _working = hotkey;
        else if (_working is { } previous) _platform.Keyboard.RegisterHotkey(previous);
        _settingsModel.HotkeyIsUnavailable = _hotkeyIsUnavailable;
        Log.Info($"hotkey: {hotkey.DisplayName} {(_hotkeyIsUnavailable ? "is used by another app" : "registered")}");
    }

    private void CreateTrayIcon()
    {
        using var stream = AssetLoader.Open(new Uri("avares://Insert/Assets/Insert.ico"));
        _pauseItem = new NativeMenuItem(_preferences.Data.CapturePaused ? "Resume Capture" : "Pause Capture");
        _pauseItem.Click += (_, _) => _preferences.Update(data => data with { CapturePaused = !data.CapturePaused });

        var menu = new NativeMenu();
        menu.Add(Item("Show Insert", () => _tray.ShowTray()));
        menu.Add(_pauseItem);
        menu.Add(Item("Clear History…", ConfirmClearHistory));
        menu.Add(new NativeMenuItemSeparator());
        menu.Add(Item("Settings…", ShowSettings));
        menu.Add(new NativeMenuItemSeparator());
        menu.Add(Item("Quit Insert", Quit));

        _trayIcon = new TrayIcon
        {
            Icon = new WindowIcon(stream),
            ToolTipText = _preferences.Data.CapturePaused ? "Insert, capture is paused" : "Insert",
            Menu = menu,
            IsVisible = _preferences.Data.ShowTrayIcon,
        };
        _trayIcon.Clicked += (_, _) => _tray.ShowTray();
        TrayIcon.SetIcons(this, [_trayIcon]);
    }

    private static NativeMenuItem Item(string header, Action action)
    {
        var item = new NativeMenuItem(header);
        item.Click += (_, _) => action();
        return item;
    }

    private void HandleCommand(string line)
    {
        var parts = line.Split(' ', 2);
        var argument = parts.Length > 1 ? parts[1] : "";
        Log.Info($"command: {line}");
        switch (parts[0])
        {
            case "show" or "start":
                _tray.ShowTray();
                break;
            case "toggle":
                _tray.Toggle();
                break;
            case "settings":
                ShowSettings();
                break;
            case "quit":
                Quit();
                break;
            case "ring":
                if (!_ring.IsShown) _ring.ShowRing();
                if (int.TryParse(argument, out var segment)) _ring.Select(segment - 1);
                break;
            case "copy" when _platform is FakePlatform fake:
                fake.SimulateCopy(ClipPayload.FromText(argument));
                break;
            case "pointer" when _platform is FakePlatform fake && argument.Split(',') is [var x, var y]:
                fake.Pointer = new PixelPoint(int.Parse(x), int.Parse(y));
                break;
        }
    }

    private void ShowSettings()
    {
        _tray.HideTray();
        if (_settings is null)
        {
            _settings = new SettingsWindow
            {
                DataContext = _settingsModel,
                RecordingChanged = recording =>
                {
                    if (recording)
                    {
                        _platform.Keyboard.RegisterHotkey(new Hotkey(ModifierKeys.None, 0));
                        _registered = null;
                    }
                    else
                    {
                        RegisterHotkey();
                    }
                },
                ClearHistoryRequested = ConfirmClearHistory,
            };
        }
        _settingsModel.LaunchAtLogin = _platform.LoginItem.IsEnabled;
        _settings.Show();
        _platform.Windows.Activate(_settings);
    }

    private async void ConfirmClearHistory()
    {
        _tray.HideTray();
        if (await new ConfirmWindow().Ask())
        {
            _library.ClearHistory();
            Log.Info("history: cleared");
        }
    }

    private void Save()
    {
        try
        {
            _library.Flush();
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            Log.Info($"save: {error.Message}");
        }
    }

    private void Quit() => _desktop?.Shutdown();

    private void Stop()
    {
        Save();
        _platform.Dispose();
        Channel?.Dispose();
        Log.Info("stop");
    }
}
