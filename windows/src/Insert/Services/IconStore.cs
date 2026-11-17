using Avalonia;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using Avalonia.Platform;
using Insert.Core;
using Insert.Platform;

namespace Insert.Services;

/// <summary>App icons and the header tint that comes from each icon. Each app is read one time.</summary>
public sealed class IconStore(IAppService apps)
{
    public sealed record Entry(Bitmap? Icon, Color Tint);

    private static readonly Entry Neutral = new(null, ToColor(Tint.Neutral));
    private readonly Dictionary<string, Entry> _entries = new(StringComparer.OrdinalIgnoreCase);
    private readonly Dictionary<string, Bitmap?> _fileIcons = new(StringComparer.OrdinalIgnoreCase);

    public Entry For(SourceApp? app)
    {
        if (app is null) return Neutral;
        if (_entries.TryGetValue(app.Id, out var entry)) return entry;
        var pixels = apps.AppIcon(app);
        entry = pixels is null ? Neutral : new Entry(ToBitmap(pixels), ToColor(Tint.FromBgra(pixels.Bgra, pixels.Width, pixels.Height)));
        _entries[app.Id] = entry;
        return entry;
    }

    public Bitmap? FileIcon(string path)
    {
        if (_fileIcons.TryGetValue(path, out var icon)) return icon;
        icon = apps.FileIcon(path) is { } pixels ? ToBitmap(pixels) : null;
        _fileIcons[path] = icon;
        return icon;
    }

    public static Color ToColor(Rgb rgb) => Color.FromRgb(rgb.R, rgb.G, rgb.B);

    public static Bitmap ToBitmap(IconPixels pixels)
    {
        var bitmap = new WriteableBitmap(new Avalonia.PixelSize(pixels.Width, pixels.Height), new Vector(96, 96), PixelFormat.Bgra8888, AlphaFormat.Unpremul);
        using var buffer = bitmap.Lock();
        for (var row = 0; row < pixels.Height; row++)
        {
            System.Runtime.InteropServices.Marshal.Copy(pixels.Bgra, row * pixels.Width * 4, buffer.Address + row * buffer.RowBytes, pixels.Width * 4);
        }
        return bitmap;
    }
}
