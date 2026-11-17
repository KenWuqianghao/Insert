namespace Insert.Core.Tests;

public sealed class PreferencesTests : IDisposable
{
    private readonly TempDirectory _dir = new();

    public void Dispose() => _dir.Dispose();

    private PreferencesStore Store => new(_dir.File("settings", "preferences.json"));

    private static void AssertSameData(PreferencesData expected, PreferencesData actual)
    {
        Assert.Equal(expected.IgnoredApps, actual.IgnoredApps);
        Assert.Equal(expected, actual with { IgnoredApps = expected.IgnoredApps });
    }

    [Fact]
    public void MissingOrUnreadableFileGivesDefaults()
    {
        AssertSameData(new PreferencesData(), Store.Load());

        Directory.CreateDirectory(_dir.File("settings"));
        File.WriteAllText(Store.Path, "{ not json");
        AssertSameData(new PreferencesData(), Store.Load());
    }

    [Fact]
    public void DataRoundTripsThroughTheFile()
    {
        var data = new PreferencesData
        {
            Hotkey = new Hotkey(ModifierKeys.Alt | ModifierKeys.Windows, 0x70),
            RingModifier = RingModifier.Windows,
            PasteDirectly = false,
            HistoryLimit = 5_000,
            CapturePaused = true,
            IgnoredApps = [new IgnoredApp(@"C:\Program Files\KeePass\KeePass.exe", "KeePass")],
            ShowTrayIcon = false,
        };

        Store.Save(data);

        AssertSameData(data, Store.Load());
    }

    [Fact]
    public void UpdateSavesAndRaisesChangedOnlyForANewValue()
    {
        var preferences = new Preferences(Store);
        var changes = new List<PreferencesData>();
        preferences.Changed += changes.Add;

        preferences.Update(data => data with { CapturePaused = true });
        preferences.Update(data => data with { CapturePaused = true });

        Assert.Single(changes);
        Assert.True(Store.Load().CapturePaused);
        Assert.True(new Preferences(Store).Data.CapturePaused);
    }

    [Fact]
    public void OutOfRangeValuesNormalizeToDefaults()
    {
        Directory.CreateDirectory(_dir.File("settings"));
        File.WriteAllText(Store.Path, """{ "hotkey": { "modifiers": "None", "key": 86 }, "historyLimit": 7, "ignoredApps": null }""");

        var loaded = Store.Load();

        Assert.Equal(Hotkey.Default, loaded.Hotkey);
        Assert.Equal(ClipLibrary.DefaultHistoryLimit, loaded.HistoryLimit);
        Assert.Empty(loaded.IgnoredApps);
        Assert.Equal(Hotkey.Default, new PreferencesData { Hotkey = new Hotkey(ModifierKeys.Control, 0xFF) }.Normalized().Hotkey);
    }

    [Fact]
    public void HotkeyDisplayNames()
    {
        Assert.Equal("Ctrl+Shift+V", Hotkey.Default.DisplayName);
        Assert.Equal("Ctrl+Win+Alt+Shift+F12", new Hotkey(ModifierKeys.Shift | ModifierKeys.Alt | ModifierKeys.Windows | ModifierKeys.Control, 0x7B).DisplayName);
    }
}
