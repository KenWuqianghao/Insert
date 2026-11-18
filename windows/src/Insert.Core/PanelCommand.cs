namespace Insert.Core;

public enum PasteAction
{
    Paste,
    PastePlainText,
    Copy,
}

/// <summary>The keys that the tray reads. The app maps its key events to these.</summary>
public enum TrayKey
{
    Other,
    Left,
    Right,
    Up,
    Down,
    Enter,
    Tab,
    Escape,
    Backspace,
    Delete,
    Space,
    /// <summary>A letter, a digit, or a punctuation key. <see cref="KeyPress.Character"/> holds it.</summary>
    Character,
}

/// <param name="Character">The character of the key without Shift, in lowercase, for <see cref="TrayKey.Character"/>.</param>
public readonly record struct KeyPress(TrayKey Key, ModifierKeys Modifiers, char Character = '\0');

public abstract record PanelCommand
{
    public sealed record Move(int Offset) : PanelCommand;
    public sealed record Extend(int Offset) : PanelCommand;
    public sealed record SelectAll : PanelCommand;
    public sealed record Activate(PasteAction Action) : PanelCommand;
    public sealed record ActivateIndex(int Index) : PanelCommand;
    public sealed record DeleteSelection : PanelCommand;
    public sealed record TogglePreview : PanelCommand;
    public sealed record Cancel : PanelCommand;
    public sealed record SwitchScope(int Offset) : PanelCommand;
    public sealed record OpenSettings : PanelCommand;
    public sealed record Hide : PanelCommand;

    /// <summary>The command for a key press in the tray, or null when the search field gets the key.</summary>
    public static PanelCommand? From(KeyPress press, PanelMode mode, bool queryIsEmpty)
    {
        var modifiers = press.Modifiers & ~ModifierKeys.Windows;
        var character = press.Key == TrayKey.Character ? char.ToLowerInvariant(press.Character) : '\0';

        // The tray takes the keyboard while another app waits for the paste. Ctrl+Q and Ctrl+W must not quit Insert.
        if (modifiers == ModifierKeys.Control && character is 'q' or 'w') return new Hide();

        if (mode is PanelMode.Naming) return press.Key == TrayKey.Escape ? new Cancel() : null;

        if (modifiers == ModifierKeys.Control)
        {
            return (press.Key, character) switch
            {
                (TrayKey.Character, 'c') => new Activate(PasteAction.Copy),
                (TrayKey.Character, 'a') => new SelectAll(),
                (TrayKey.Character, ',') => new OpenSettings(),
                (TrayKey.Character, '[') => new SwitchScope(-1),
                (TrayKey.Character, ']') => new SwitchScope(1),
                (TrayKey.Backspace or TrayKey.Delete, _) => new DeleteSelection(),
                (TrayKey.Character, >= '1' and <= '9') => new ActivateIndex(character - '1'),
                _ => null,
            };
        }

        var isPlain = modifiers == ModifierKeys.None;
        var isShift = modifiers == ModifierKeys.Shift;
        if (!isPlain && !isShift) return null;

        return press.Key switch
        {
            TrayKey.Left or TrayKey.Up => isShift ? new Extend(-1) : new Move(-1),
            TrayKey.Right or TrayKey.Down => isShift ? new Extend(1) : new Move(1),
            TrayKey.Enter => new Activate(isShift ? PasteAction.PastePlainText : PasteAction.Paste),
            TrayKey.Tab => new SwitchScope(isShift ? -1 : 1),
            TrayKey.Escape when isPlain => new Cancel(),
            TrayKey.Backspace or TrayKey.Delete when isPlain && queryIsEmpty => new DeleteSelection(),
            TrayKey.Space when isPlain && (queryIsEmpty || mode is PanelMode.Previewing) => new TogglePreview(),
            _ => null,
        };
    }
}
