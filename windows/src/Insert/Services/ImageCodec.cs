using Avalonia;
using Avalonia.Media.Imaging;
using Insert.Core;

namespace Insert.Services;

/// <summary>Reads images with the Skia decoders of Avalonia: PNG, JPEG, BMP, GIF, and WebP.</summary>
public sealed class ImageCodec : IImageCodec
{
    public (Core.PixelSize Size, byte[] ThumbnailPng)? Inspect(byte[] encoded, int maxThumbnailSide)
    {
        try
        {
            using var bitmap = new Bitmap(new MemoryStream(encoded));
            var size = bitmap.PixelSize;
            if (size.Width <= 0 || size.Height <= 0) return null;
            var scale = Math.Min(1.0, (double)maxThumbnailSide / Math.Max(size.Width, size.Height));
            var target = new Avalonia.PixelSize(Math.Max(1, (int)Math.Round(size.Width * scale)), Math.Max(1, (int)Math.Round(size.Height * scale)));
            using var thumbnail = bitmap.CreateScaledBitmap(target, BitmapInterpolationMode.HighQuality);
            using var output = new MemoryStream();
            thumbnail.Save(output, PngBitmapEncoderOptions.Default);
            return (new Core.PixelSize(size.Width, size.Height), output.ToArray());
        }
        catch (Exception error) when (error is ArgumentException or InvalidOperationException or NotSupportedException or IOException)
        {
            Log.Info($"image: cannot decode {encoded.Length} bytes: {error.Message}");
            return null;
        }
    }
}
