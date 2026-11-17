namespace Insert.Core.Tests;

/// <summary>A new empty directory for one test. Dispose deletes it.</summary>
internal sealed class TempDirectory : IDisposable
{
    public string Path { get; } = System.IO.Path.Combine(System.IO.Path.GetTempPath(), $"insert-tests-{Guid.NewGuid():N}");

    public TempDirectory() => Directory.CreateDirectory(Path);

    public string File(params string[] parts) => System.IO.Path.Combine([Path, .. parts]);

    public void Dispose()
    {
        if (Directory.Exists(Path)) Directory.Delete(Path, recursive: true);
    }
}

/// <summary>An image codec that reads every input as an image of one fixed size, or as no image.</summary>
internal sealed class FakeImageCodec((PixelSize Size, byte[] ThumbnailPng)? result = null) : IImageCodec
{
    public byte[]? LastInput { get; private set; }

    public (PixelSize Size, byte[] ThumbnailPng)? Inspect(byte[] encoded, int maxThumbnailSide)
    {
        LastInput = encoded;
        return result;
    }
}

internal static class Fixtures
{
    public static readonly DateTimeOffset Now = new(2026, 1, 2, 3, 4, 5, TimeSpan.Zero);

    public static CapturedClip Capture(string text, SourceApp? source = null, DateTimeOffset? at = null) =>
        ClipCapture.Build(ClipPayload.FromText(text), source, new FakeImageCodec(), at ?? Now)
        ?? throw new InvalidOperationException($"No clip for \"{text}\"");

    public static Clip Clip(ClipContent content, Guid? pinboard = null, string? title = null, SourceApp? source = null) =>
        new(Guid.NewGuid(), Now, source, content, title, pinboard, Guid.NewGuid().ToString("N"), 10);
}
