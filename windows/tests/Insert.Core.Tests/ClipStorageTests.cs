using System.Text;

namespace Insert.Core.Tests;

public sealed class ClipStorageTests : IDisposable
{
    private readonly TempDirectory _dir = new();

    public void Dispose() => _dir.Dispose();

    [Fact]
    public void PayloadEncodeDecodeRoundTrip()
    {
        var payload = new ClipPayload([
            new Representation(ClipFormats.UnicodeText, Encoding.Unicode.GetBytes("héllo")),
            new Representation(ClipFormats.Html, []),
            new Representation("Format ✓", [0, 255, 7]),
        ]);

        var decoded = ClipStorage.DecodePayload(ClipStorage.EncodePayload(payload));

        Assert.NotNull(decoded);
        Assert.Equal(payload.Representations.Select(r => r.Format), decoded.Representations.Select(r => r.Format));
        Assert.Equal(payload.Representations.Select(r => r.Data), decoded.Representations.Select(r => r.Data));
        Assert.Equal(payload.ContentHash, decoded.ContentHash);
    }

    [Fact]
    public void DecodeRejectsWrongMagicAndTruncatedData()
    {
        var encoded = ClipStorage.EncodePayload(ClipPayload.FromText("text"));
        Assert.Null(ClipStorage.DecodePayload([.. "NOPE"u8, .. encoded[4..]]));
        Assert.Null(ClipStorage.DecodePayload(encoded[..^1]));
        Assert.Null(ClipStorage.DecodePayload([]));
    }

    [Fact]
    public void WriteStoresPayloadAndThumbnailAndRemoveFilesDeletesThem()
    {
        var storage = new ClipStorage(_dir.Path);
        var capture = Fixtures.Capture("text") with { Thumbnail = [9, 9] };

        storage.Write(capture);

        var id = capture.Clip.Id;
        Assert.True(storage.HasPayload(id));
        Assert.Equal("text", storage.LoadPayload(id)!.Text);
        Assert.Equal([9, 9], File.ReadAllBytes(storage.ThumbnailPath(id)));
        Assert.Empty(Directory.GetFiles(_dir.Path, "*.tmp", SearchOption.AllDirectories));

        storage.RemoveFiles(id);
        Assert.False(storage.HasPayload(id));
        Assert.False(File.Exists(storage.ThumbnailPath(id)));
        Assert.Null(storage.LoadPayload(id));
    }

    [Fact]
    public void MissingIndexLoadsEmpty()
    {
        Assert.Same(LibraryIndex.Empty, new ClipStorage(_dir.Path).LoadIndex());
    }
}
