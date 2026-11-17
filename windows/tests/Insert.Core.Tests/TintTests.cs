namespace Insert.Core.Tests;

public sealed class TintTests
{
    private static byte[] Icon(byte r, byte g, byte b, byte a) =>
        Enumerable.Repeat(new[] { r, g, b, a }, 24 * 24).SelectMany(pixel => pixel).ToArray();

    private static (double Hue, double Brightness) Hsb(Rgb color)
    {
        double r = color.R / 255.0, g = color.G / 255.0, b = color.B / 255.0;
        var max = Math.Max(r, Math.Max(g, b));
        var chroma = max - Math.Min(r, Math.Min(g, b));
        if (chroma == 0) return (0, max);
        var hue = max == r ? (g - b) / chroma % 6 : max == g ? (b - r) / chroma + 2 : (r - g) / chroma + 4;
        return ((hue < 0 ? hue + 6 : hue) / 6, max);
    }

    [Fact]
    public void TransparentIconGivesTheNeutralTint()
    {
        Assert.Equal(Tint.Neutral, Tint.FromRgba(Icon(255, 0, 0, 100)));
        Assert.Equal(Tint.Neutral, Tint.FromRgba([]));
    }

    [Fact]
    public void GrayIconGivesAClampedGray()
    {
        Assert.Equal(new Rgb(117, 117, 117), Tint.FromRgba(Icon(255, 255, 255, 255)));
        Assert.Equal(new Rgb(66, 66, 66), Tint.FromRgba(Icon(0, 0, 0, 255)));
        Assert.Equal(new Rgb(70, 70, 70), Tint.FromRgba(Icon(128, 128, 128, 255)));
    }

    [Fact]
    public void SaturatedRedKeepsItsHueWithBrightnessAtMostPointEight()
    {
        var tint = Tint.FromRgba(Icon(255, 0, 0, 255));

        Assert.Equal(new Rgb(204, 29, 29), tint);
        var (hue, brightness) = Hsb(tint);
        Assert.Equal(0, hue, 3);
        Assert.True(brightness <= 0.8 + 1e-9);
    }

    [Fact]
    public void BrightHueIsCappedAtBrightnessPointSixSix()
    {
        var tint = Tint.FromRgba(Icon(255, 230, 0, 255));

        Assert.Equal(168, tint.R);
        Assert.True(tint.G > tint.B);
        Assert.True(Hsb(tint).Brightness <= 0.66 + 0.5 / 255);
    }

    [Fact]
    public void DominantHueBucketWins()
    {
        var pixels = Icon(0, 0, 255, 255);
        for (var index = 0; index < 100; index++) pixels.AsSpan(index * 4, 4).SetTo([0, 200, 0, 255]);

        var tint = Tint.FromRgba(pixels);

        Assert.True(tint.B > tint.G && tint.B > tint.R);
    }

    [Fact]
    public void FromBgraReadsBlueGreenRedAlpha()
    {
        var bgra = Enumerable.Repeat(new byte[] { 0, 0, 255, 255 }, 48 * 48).SelectMany(pixel => pixel).ToArray();

        Assert.Equal(new Rgb(204, 29, 29), Tint.FromBgra(bgra, 48, 48));
        Assert.Equal(Tint.Neutral, Tint.FromBgra(bgra, 48, 49));
    }
}

internal static class SpanExtensions
{
    public static void SetTo(this Span<byte> target, ReadOnlySpan<byte> source) => source.CopyTo(target);
}
