using System.Text.Json;

namespace Insert.Core;

/// <summary>The modifier keys. The values are the <c>MOD_</c> flags of <c>RegisterHotKey</c>.</summary>
[Flags]
public enum ModifierKeys
{
    None = 0,
    Alt = 0x1,
    Control = 0x2,
    Shift = 0x4,
    Windows = 0x8,
}

/// <summary>The modifier key that opens the clip ring when the user holds it alone.</summary>
public enum RingModifier
{
    Alt,
    Control,
    Windows,
}

public static class RingModifiers
{
    public static readonly IReadOnlyList<RingModifier> All = Enum.GetValues<RingModifier>();

    public static ModifierKeys Key(this RingModifier modifier) => modifier switch
    {
        RingModifier.Alt => ModifierKeys.Alt,
        RingModifier.Control => ModifierKeys.Control,
        RingModifier.Windows => ModifierKeys.Windows,
        _ => throw new ArgumentOutOfRangeException(nameof(modifier)),
    };

    public static string Name(this RingModifier modifier) => modifier switch
    {
        RingModifier.Alt => "Alt",
        RingModifier.Control => "Ctrl",
        RingModifier.Windows => "Windows",
        _ => throw new ArgumentOutOfRangeException(nameof(modifier)),
    };
}

/// <summary>A global shortcut: modifier keys and one Windows virtual-key code.</summary>
public sealed record Hotkey(ModifierKeys Modifiers, int Key)
{
    public const int VkV = 0x56;

    public static readonly Hotkey Default = new(ModifierKeys.Control | ModifierKeys.Shift, VkV);

    /// <summary>A shortcut needs one modifier and a key with a name.</summary>
    public bool IsValid => Modifiers != ModifierKeys.None && KeyNames.Name(Key) is not null;

    public string DisplayName
    {
        get
        {
            var parts = new List<string>();
            if (Modifiers.HasFlag(ModifierKeys.Control)) parts.Add("Ctrl");
            if (Modifiers.HasFlag(ModifierKeys.Windows)) parts.Add("Win");
            if (Modifiers.HasFlag(ModifierKeys.Alt)) parts.Add("Alt");
            if (Modifiers.HasFlag(ModifierKeys.Shift)) parts.Add("Shift");
            parts.Add(KeyNames.Name(Key) ?? $"Key {Key}");
            return string.Join('+', parts);
        }
    }
}

/// <summary>The names of the virtual-key codes that a shortcut can use.</summary>
public static class KeyNames
{
    private static readonly Dictionary<int, string> Names = Build();

    public static string? Name(int virtualKey) => Names.GetValueOrDefault(virtualKey);

    private static Dictionary<int, string> Build()
    {
        var names = new Dictionary<int, string>
        {
            [0x08] = "Backspace", [0x09] = "Tab", [0x0D] = "Enter", [0x1B] = "Esc", [0x20] = "Space",
            [0x21] = "Page Up", [0x22] = "Page Down", [0x23] = "End", [0x24] = "Home",
            [0x25] = "Left", [0x26] = "Up", [0x27] = "Right", [0x28] = "Down",
            [0x2D] = "Insert", [0x2E] = "Delete",
            [0xBA] = ";", [0xBB] = "=", [0xBC] = ",", [0xBD] = "-", [0xBE] = ".", [0xBF] = "/", [0xC0] = "`",
            [0xDB] = "[", [0xDC] = "\\", [0xDD] = "]", [0xDE] = "'",
        };
        for (var key = '0'; key <= '9'; key++) names[key] = key.ToString();
        for (var key = 'A'; key <= 'Z'; key++) names[key] = key.ToString();
        for (var number = 0; number <= 9; number++) names[0x60 + number] = $"Num {number}";
        for (var number = 1; number <= 24; number++) names[0x6F + number] = $"F{number}";
        return names;
    }
}

public sealed record IgnoredApp(string Id, string Name);

/// <summary>The settings in <c>preferences.json</c>. Launch at login is in the registry, so it is not here.</summary>
public sealed record PreferencesData
{
    public Hotkey Hotkey { get; init; } = Hotkey.Default;
    public RingModifier RingModifier { get; init; } = RingModifier.Alt;
    public bool PasteDirectly { get; init; } = true;
    public int HistoryLimit { get; init; } = ClipLibrary.DefaultHistoryLimit;
    public bool CapturePaused { get; init; }
    public IReadOnlyList<IgnoredApp> IgnoredApps { get; init; } = [];
    public bool ShowTrayIcon { get; init; } = true;

    /// <summary>Replaces values that are out of range with the defaults.</summary>
    public PreferencesData Normalized() => this with
    {
        Hotkey = Hotkey is { IsValid: true } ? Hotkey : Hotkey.Default,
        HistoryLimit = ClipLibrary.HistoryLimitOptions.Contains(HistoryLimit) ? HistoryLimit : ClipLibrary.DefaultHistoryLimit,
        IgnoredApps = IgnoredApps ?? [],
    };
}

public sealed class PreferencesStore(string path)
{
    public string Path { get; } = path;

    public PreferencesData Load()
    {
        try
        {
            var text = File.ReadAllText(Path);
            return (JsonSerializer.Deserialize(text, InsertJson.Default.PreferencesData) ?? new PreferencesData()).Normalized();
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException or JsonException)
        {
            return new PreferencesData();
        }
    }

    public void Save(PreferencesData data)
    {
        Directory.CreateDirectory(System.IO.Path.GetDirectoryName(Path)!);
        var temporary = Path + ".tmp";
        File.WriteAllText(temporary, JsonSerializer.Serialize(data, InsertJson.Default.PreferencesData));
        File.Move(temporary, Path, overwrite: true);
    }
}

/// <summary>The settings that the app uses. Each change saves the file and raises <see cref="Changed"/>.</summary>
public sealed class Preferences(PreferencesStore store)
{
    private PreferencesData _data = store.Load();

    public event Action<PreferencesData>? Changed;

    public PreferencesData Data => _data;

    public void Update(Func<PreferencesData, PreferencesData> change)
    {
        var next = change(_data).Normalized();
        if (next == _data) return;
        _data = next;
        store.Save(next);
        Changed?.Invoke(next);
    }
}
