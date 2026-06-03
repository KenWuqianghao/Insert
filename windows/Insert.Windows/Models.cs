using System;
using System.Collections.Generic;
using System.Drawing;
using System.Windows.Forms;

namespace Insert.Windows;

enum ClipboardKind
{
    Text,
    Image,
    Files,
    Html,
    Rtf,
    Url,
    Audio,
    Video,
    Color,
    Data
}

sealed record ClipboardEntry(
    Guid Id,
    DateTimeOffset CreatedAt,
    string SourceApp,
    ClipboardKind Kind,
    string Title,
    string Preview,
    string SearchText,
    ClipboardPayload Payload
);

sealed record ClipboardPayload(
    string? Text,
    string? Html,
    string? Rtf,
    string? Url,
    IReadOnlyList<string>? Files,
    string? ImageBase64,
    int? ImageWidth,
    int? ImageHeight,
    string? DataBase64,
    string? DataFormat,
    IReadOnlyList<ClipboardRepresentation>? ExtraRepresentations
);

sealed record ClipboardRepresentation(string Format, string? Text, string? Base64);

sealed record HotkeyBinding(Keys Key, HotkeyModifiers Modifiers)
{
    public string DisplayText => HotkeyFormatter.ToDisplayText(this);
}

static class HotkeyFormatter
{
    public static string ToDisplayText(HotkeyBinding binding)
    {
        var parts = "";
        if (binding.Modifiers.HasFlag(HotkeyModifiers.Control)) parts += "Ctrl+";
        if (binding.Modifiers.HasFlag(HotkeyModifiers.Alt)) parts += "Alt+";
        if (binding.Modifiers.HasFlag(HotkeyModifiers.Shift)) parts += "Shift+";
        if (binding.Modifiers.HasFlag(HotkeyModifiers.Win)) parts += "Win+";
        return parts + binding.Key.ToString().ToUpperInvariant();
    }
}

sealed record HotkeySettings(HotkeyBinding Binding)
{
    public static HotkeySettings Default => new(new HotkeyBinding(Keys.V, HotkeyModifiers.Control | HotkeyModifiers.Alt));
}
