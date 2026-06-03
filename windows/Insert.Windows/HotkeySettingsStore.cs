using System;
using System.IO;
using System.Text.Json;
using System.Windows.Forms;

namespace Insert.Windows;

sealed class HotkeySettingsStore
{
    private readonly string _path;
    private readonly JsonSerializerOptions _jsonOptions = new() { WriteIndented = true };

    public HotkeySettingsStore()
    {
        var appData = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
        var dir = Path.Combine(appData, "Insert");
        Directory.CreateDirectory(dir);
        _path = Path.Combine(dir, "HotkeySettings.json");
    }

    public HotkeySettings Load()
    {
        try
        {
            if (!File.Exists(_path))
            {
                return HotkeySettings.Default;
            }

            var json = File.ReadAllText(_path);
            return JsonSerializer.Deserialize<HotkeySettings>(json, _jsonOptions) ?? HotkeySettings.Default;
        }
        catch
        {
            return HotkeySettings.Default;
        }
    }

    public void Save(HotkeySettings settings)
    {
        try
        {
            File.WriteAllText(_path, JsonSerializer.Serialize(settings, _jsonOptions));
        }
        catch
        {
        }
    }
}

