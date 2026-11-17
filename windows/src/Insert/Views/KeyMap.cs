using Avalonia.Input;
using Insert.Core;
using Insert.Platform;

namespace Insert.Views;

/// <summary>Maps the key events of Avalonia to the keys of the core and to Windows virtual-key codes.</summary>
public static class KeyMap
{
    public static ModifierKeys Modifiers(KeyModifiers modifiers) =>
        (modifiers.HasFlag(KeyModifiers.Control) ? ModifierKeys.Control : 0)
        | (modifiers.HasFlag(KeyModifiers.Shift) ? ModifierKeys.Shift : 0)
        | (modifiers.HasFlag(KeyModifiers.Alt) ? ModifierKeys.Alt : 0)
        | (modifiers.HasFlag(KeyModifiers.Meta) ? ModifierKeys.Windows : 0);

    public static KeyPress Press(Key key, KeyModifiers modifiers)
    {
        var mods = Modifiers(modifiers);
        return key switch
        {
            Key.Left => new KeyPress(TrayKey.Left, mods),
            Key.Right => new KeyPress(TrayKey.Right, mods),
            Key.Up => new KeyPress(TrayKey.Up, mods),
            Key.Down => new KeyPress(TrayKey.Down, mods),
            Key.Enter => new KeyPress(TrayKey.Enter, mods),
            Key.Tab => new KeyPress(TrayKey.Tab, mods),
            Key.Escape => new KeyPress(TrayKey.Escape, mods),
            Key.Back => new KeyPress(TrayKey.Backspace, mods),
            Key.Delete => new KeyPress(TrayKey.Delete, mods),
            Key.Space => new KeyPress(TrayKey.Space, mods),
            _ when Character(key) is { } c => new KeyPress(TrayKey.Character, mods, c),
            _ => new KeyPress(TrayKey.Other, mods),
        };
    }

    public static RingKey Ring(Key key) => key switch
    {
        Key.Escape => new RingKey(RingKeyKind.Escape),
        Key.Enter => new RingKey(RingKeyKind.Enter),
        >= Key.D1 and <= Key.D9 => new RingKey(RingKeyKind.Digit, key - Key.D0),
        >= Key.NumPad1 and <= Key.NumPad9 => new RingKey(RingKeyKind.Digit, key - Key.NumPad0),
        _ => new RingKey(RingKeyKind.Other),
    };

    /// <summary>The character of a letter, digit, or punctuation key without Shift, in lowercase.</summary>
    public static char? Character(Key key) => key switch
    {
        >= Key.A and <= Key.Z => (char)('a' + (key - Key.A)),
        >= Key.D0 and <= Key.D9 => (char)('0' + (key - Key.D0)),
        >= Key.NumPad0 and <= Key.NumPad9 => (char)('0' + (key - Key.NumPad0)),
        Key.OemComma => ',',
        Key.OemPeriod => '.',
        Key.OemOpenBrackets => '[',
        Key.OemCloseBrackets => ']',
        Key.OemMinus => '-',
        Key.OemPlus => '=',
        Key.OemSemicolon => ';',
        Key.OemQuotes => '\'',
        Key.OemQuestion => '/',
        Key.OemPipe => '\\',
        Key.OemTilde => '`',
        _ => null,
    };

    /// <summary>The Windows virtual-key code of a key that a global shortcut can use.</summary>
    public static int? VirtualKey(Key key) => key switch
    {
        >= Key.A and <= Key.Z => 0x41 + (key - Key.A),
        >= Key.D0 and <= Key.D9 => 0x30 + (key - Key.D0),
        >= Key.NumPad0 and <= Key.NumPad9 => 0x60 + (key - Key.NumPad0),
        >= Key.F1 and <= Key.F24 => 0x70 + (key - Key.F1),
        Key.Back => 0x08,
        Key.Tab => 0x09,
        Key.Enter => 0x0D,
        Key.Space => 0x20,
        Key.PageUp => 0x21,
        Key.PageDown => 0x22,
        Key.End => 0x23,
        Key.Home => 0x24,
        Key.Left => 0x25,
        Key.Up => 0x26,
        Key.Right => 0x27,
        Key.Down => 0x28,
        Key.Insert => 0x2D,
        Key.Delete => 0x2E,
        Key.OemSemicolon => 0xBA,
        Key.OemPlus => 0xBB,
        Key.OemComma => 0xBC,
        Key.OemMinus => 0xBD,
        Key.OemPeriod => 0xBE,
        Key.OemQuestion => 0xBF,
        Key.OemTilde => 0xC0,
        Key.OemOpenBrackets => 0xDB,
        Key.OemPipe => 0xDC,
        Key.OemCloseBrackets => 0xDD,
        Key.OemQuotes => 0xDE,
        _ => null,
    };

    public static bool IsModifier(Key key) =>
        key is Key.LeftCtrl or Key.RightCtrl or Key.LeftShift or Key.RightShift or Key.LeftAlt or Key.RightAlt or Key.LWin or Key.RWin;
}
