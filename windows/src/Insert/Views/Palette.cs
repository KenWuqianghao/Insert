using Avalonia;
using Avalonia.Controls;
using Avalonia.Media;
using Insert.Core;

namespace Insert.Views;

public static class Metrics
{
    public const double TopBarHeight = 54;
    public const double CardSize = 232;
    public const double CardCornerRadius = 14;
    public const double StripHeight = CardSize + 38;
    public const double BaseHeight = TopBarHeight + StripHeight;
    public const double PreviewMaxHeight = 480;
    public const double PreviewScreenFraction = 0.46;
}

public static class Palette
{
    public static Color PinboardColor(PinboardColor color) => color switch
    {
        Core.PinboardColor.Red => Color.FromRgb(0xFF, 0x3B, 0x30),
        Core.PinboardColor.Orange => Color.FromRgb(0xFF, 0x95, 0x00),
        Core.PinboardColor.Yellow => Color.FromRgb(0xFF, 0xCC, 0x00),
        Core.PinboardColor.Green => Color.FromRgb(0x34, 0xC7, 0x59),
        Core.PinboardColor.Teal => Color.FromRgb(0x30, 0xB0, 0xC7),
        Core.PinboardColor.Blue => Color.FromRgb(0x00, 0x7A, 0xFF),
        Core.PinboardColor.Purple => Color.FromRgb(0xAF, 0x52, 0xDE),
        Core.PinboardColor.Pink => Color.FromRgb(0xFF, 0x2D, 0x55),
        _ => Colors.Gray,
    };

    public static IBrush PinboardBrush(PinboardColor color) => new SolidColorBrush(PinboardColor(color));

    public static Color ToColor(ColorValue value) => Color.FromArgb(
        (byte)Math.Round(value.Alpha * 255), (byte)Math.Round(value.Red * 255), (byte)Math.Round(value.Green * 255), (byte)Math.Round(value.Blue * 255));

    /// <summary>The icon of a clip kind, from <c>Assets/Icons.axaml</c>.</summary>
    public static Geometry? KindIcon(ClipKind kind) => Icon(kind switch
    {
        ClipKind.Text => "TextAlignLeft",
        ClipKind.RichText => "TextFont",
        ClipKind.Link => "Link",
        ClipKind.Image => "Image",
        ClipKind.Files => "Document",
        ClipKind.Color => "Palette",
        ClipKind.Pdf => "DocumentPdf",
        ClipKind.Media => "Video",
        _ => "Box",
    });

    public static Geometry? Icon(string name) =>
        Application.Current!.TryGetResource($"Icon.{name}", null, out var value) ? value as Geometry : null;
}
