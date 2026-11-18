using System.Collections.ObjectModel;
using Avalonia.Media.Imaging;
using CommunityToolkit.Mvvm.ComponentModel;
using Insert.Core;
using Insert.Platform;
using Insert.Services;

namespace Insert.Views;

public sealed record IgnoredAppRow(IgnoredApp App, Bitmap? Icon);

public sealed record Choice<T>(T Value, string Name)
{
    public override string ToString() => Name;
}

public sealed partial class SettingsViewModel : ObservableObject
{
    private readonly Preferences _preferences;
    private readonly IPlatform _platform;
    private readonly IconStore _icons;

    public SettingsViewModel(Preferences preferences, IPlatform platform, IconStore icons)
    {
        _preferences = preferences;
        _platform = platform;
        _icons = icons;
        RingModifiers = Core.RingModifiers.All.Select(modifier => new Choice<RingModifier>(modifier, modifier.Name())).ToList();
        HistoryLimits = ClipLibrary.HistoryLimitOptions.Select(limit => new Choice<int>(limit, $"{limit:N0} clips")).ToList();
        preferences.Changed += _ => Refresh();
        Refresh();
    }

    public IReadOnlyList<Choice<RingModifier>> RingModifiers { get; }
    public IReadOnlyList<Choice<int>> HistoryLimits { get; }
    public ObservableCollection<IgnoredAppRow> IgnoredApps { get; } = [];

    [ObservableProperty]
    public partial bool IsGeneral { get; set; } = true;

    [ObservableProperty]
    public partial string? LoginItemError { get; set; }

    [ObservableProperty]
    public partial bool HotkeyIsUnavailable { get; set; }

    [ObservableProperty]
    public partial bool RingIsUnavailable { get; set; }

    [ObservableProperty]
    public partial bool IsRecording { get; set; }

    public bool IsPrivacy => !IsGeneral;

    partial void OnIsGeneralChanged(bool value) => OnPropertyChanged(nameof(IsPrivacy));

    public string HotkeyName => IsRecording ? "Press a shortcut…" : _preferences.Data.Hotkey.DisplayName;

    partial void OnIsRecordingChanged(bool value) => OnPropertyChanged(nameof(HotkeyName));

    public bool LaunchAtLogin
    {
        get => _platform.LoginItem.IsEnabled;
        set
        {
            LoginItemError = _platform.LoginItem.SetEnabled(value);
            OnPropertyChanged();
        }
    }

    public bool ShowTrayIcon
    {
        get => _preferences.Data.ShowTrayIcon;
        set => _preferences.Update(data => data with { ShowTrayIcon = value });
    }

    public Choice<RingModifier> RingModifier
    {
        get => RingModifiers.First(choice => choice.Value == _preferences.Data.RingModifier);
        set => _preferences.Update(data => data with { RingModifier = value.Value });
    }

    public bool PasteDirectly
    {
        get => _preferences.Data.PasteDirectly;
        set => _preferences.Update(data => data with { PasteDirectly = value });
    }

    public Choice<int> HistoryLimit
    {
        get => HistoryLimits.First(choice => choice.Value == _preferences.Data.HistoryLimit);
        set => _preferences.Update(data => data with { HistoryLimit = value.Value });
    }

    public bool CapturePaused
    {
        get => _preferences.Data.CapturePaused;
        set => _preferences.Update(data => data with { CapturePaused = value });
    }

    public void SetHotkey(Hotkey hotkey) => _preferences.Update(data => data with { Hotkey = hotkey });

    public void AddIgnoredApp(string path)
    {
        if (_platform.Apps.Describe(path) is not { } app) return;
        if (_preferences.Data.IgnoredApps.Any(ignored => app.SameId(ignored.Id))) return;
        _preferences.Update(data => data with { IgnoredApps = [.. data.IgnoredApps, new IgnoredApp(app.Id, app.Name)] });
    }

    public void RemoveIgnoredApp(IgnoredApp app) =>
        _preferences.Update(data => data with { IgnoredApps = data.IgnoredApps.Where(ignored => ignored != app).ToList() });

    private void Refresh()
    {
        IgnoredApps.Clear();
        foreach (var app in _preferences.Data.IgnoredApps)
            IgnoredApps.Add(new IgnoredAppRow(app, _icons.For(new SourceApp(app.Id, app.Name)).Icon));
        OnPropertyChanged(nameof(HotkeyName));
        OnPropertyChanged(nameof(ShowTrayIcon));
        OnPropertyChanged(nameof(RingModifier));
        OnPropertyChanged(nameof(PasteDirectly));
        OnPropertyChanged(nameof(HistoryLimit));
        OnPropertyChanged(nameof(CapturePaused));
        OnPropertyChanged(nameof(LaunchAtLogin));
    }
}
