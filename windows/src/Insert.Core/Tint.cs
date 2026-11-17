namespace Insert.Core;

public readonly record struct Rgb(byte R, byte G, byte B);

/// <summary>The header color of a card, from the icon of the source app.</summary>
public static class Tint
{
    public static readonly Rgb Neutral = new(0x6B, 0x6B, 0x6B);
    private const int SampleSide = 24;

    private struct Bucket
    {
        public double Weight, Red, Green, Blue;
    }

    /// <summary>Samples straight (not premultiplied) BGRA pixels at 24 × 24, then calls <see cref="FromRgba"/>.</summary>
    public static Rgb FromBgra(ReadOnlySpan<byte> bgra, int width, int height)
    {
        if (width <= 0 || height <= 0 || bgra.Length < width * height * 4) return Neutral;
        var rgba = new byte[SampleSide * SampleSide * 4];
        for (var y = 0; y < SampleSide; y++)
        {
            for (var x = 0; x < SampleSide; x++)
            {
                var source = (((y * 2 + 1) * height / (SampleSide * 2)) * width + (x * 2 + 1) * width / (SampleSide * 2)) * 4;
                var target = (y * SampleSide + x) * 4;
                rgba[target] = bgra[source + 2];
                rgba[target + 1] = bgra[source + 1];
                rgba[target + 2] = bgra[source];
                rgba[target + 3] = bgra[source + 3];
            }
        }
        return FromRgba(rgba);
    }

    /// <summary>Finds the strongest hue of the icon, then limits saturation and brightness so white text stays readable.</summary>
    public static Rgb FromRgba(ReadOnlySpan<byte> rgba)
    {
        var buckets = new Bucket[12];
        double opaque = 0, colorful = 0, valueSum = 0;

        for (var offset = 0; offset + 3 < rgba.Length; offset += 4)
        {
            if (rgba[offset + 3] / 255.0 <= 0.5) continue;
            var red = rgba[offset] / 255.0;
            var green = rgba[offset + 1] / 255.0;
            var blue = rgba[offset + 2] / 255.0;
            var value = Math.Max(red, Math.Max(green, blue));
            var chroma = value - Math.Min(red, Math.Min(green, blue));
            opaque++;
            valueSum += value;
            if (value <= 0.2 || chroma / value <= 0.3) continue;

            colorful++;
            var index = Math.Min(11, (int)(Hue(red, green, blue, value, chroma) * 12));
            buckets[index].Weight += chroma;
            buckets[index].Red += red * chroma;
            buckets[index].Green += green * chroma;
            buckets[index].Blue += blue * chroma;
        }

        if (opaque == 0) return Neutral;
        var top = buckets.MaxBy(bucket => bucket.Weight);
        if (colorful / opaque <= 0.08 || top.Weight == 0)
        {
            var level = ToByte(Math.Clamp(valueSum / opaque * 0.55, 0.26, 0.46));
            return new Rgb(level, level, level);
        }

        var (r, g, b) = (top.Red / top.Weight, top.Green / top.Weight, top.Blue / top.Weight);
        var v = Math.Max(r, Math.Max(g, b));
        var c = v - Math.Min(r, Math.Min(g, b));
        var h = c == 0 ? 0 : Hue(r, g, b, v, c);
        var s = v == 0 ? 0 : c / v;
        var isBrightHue = h is >= 0.11 and <= 0.5;
        return FromHsb(h, Math.Clamp(s, 0.5, 0.86), Math.Clamp(v, 0.48, isBrightHue ? 0.66 : 0.8));
    }

    private static double Hue(double red, double green, double blue, double value, double chroma)
    {
        double h;
        if (value == red) h = (green - blue) / chroma % 6;
        else if (value == green) h = (blue - red) / chroma + 2;
        else h = (red - green) / chroma + 4;
        return (h < 0 ? h + 6 : h) / 6;
    }

    private static Rgb FromHsb(double h, double s, double v)
    {
        var sector = (h * 6 % 6 + 6) % 6;
        var c = v * s;
        var x = c * (1 - Math.Abs(sector % 2 - 1));
        var (r, g, b) = (int)sector switch
        {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        var m = v - c;
        return new Rgb(ToByte(r + m), ToByte(g + m), ToByte(b + m));
    }

    private static byte ToByte(double value) => (byte)Math.Round(Math.Clamp(value, 0, 1) * 255);
}
