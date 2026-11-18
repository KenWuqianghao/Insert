namespace Insert.Core.Tests;

public sealed class ClipLibraryTests : IDisposable
{
    private readonly TempDirectory _dir = new();
    private DateTimeOffset _now = Fixtures.Now;

    public void Dispose() => _dir.Dispose();

    private ClipStorage Storage => new(_dir.Path);

    private ClipLibrary Open(int historyLimit = 100) => new(Storage, historyLimit, () => _now);

    private static Guid Insert(ClipLibrary library, string text, SourceApp? source = null)
    {
        library.Insert(Fixtures.Capture(text, source));
        return library.Clips[0].Id;
    }

    private static Guid[] Ids(ClipLibrary library) => library.Clips.Select(clip => clip.Id).ToArray();

    [Fact]
    public void InsertWritesFilesAndPutsTheClipFirst()
    {
        var library = Open();
        var first = Insert(library, "a");
        var changes = 0;
        library.Changed += () => changes++;
        var second = Insert(library, "b");

        Assert.Equal([second, first], Ids(library));
        Assert.Equal(1, changes);
        Assert.True(library.HasUnsavedChanges);
        Assert.True(Storage.HasPayload(second));
        Assert.Equal("b", library.Payload(second)!.Text);
    }

    [Fact]
    public void DuplicateMovesToFrontWithNewTimeAndKeepsOrUpdatesTheSource()
    {
        var library = Open();
        var original = Insert(library, "same", new SourceApp(@"C:\a.exe", "A"));
        library.Rename(original, "Title");
        Insert(library, "other");

        _now = Fixtures.Now.AddMinutes(5);
        library.Insert(Fixtures.Capture("same", new SourceApp(@"C:\b.exe", "B")));
        Assert.Equal(2, library.Clips.Count);
        var moved = library.Clips[0];
        Assert.Equal(original, moved.Id);
        Assert.Equal(_now, moved.CreatedAt);
        Assert.Equal("B", moved.Source?.Name);
        Assert.Equal("Title", moved.CustomTitle);

        library.Insert(Fixtures.Capture("same", source: null));
        Assert.Equal("B", library.Clips[0].Source?.Name);
    }

    [Fact]
    public void HistoryLimitTrimsUnpinnedClipsOnly()
    {
        var library = Open(historyLimit: 2);
        var board = library.AddPinboard("Work");
        var pinned = Insert(library, "pinned");
        library.SetPinboard(board, new HashSet<Guid> { pinned });
        var oldest = Insert(library, "a");
        var b = Insert(library, "b");
        var c = Insert(library, "c");

        Assert.Equal([c, b, pinned], Ids(library));
        library.Flush();
        Assert.False(Storage.HasPayload(oldest));

        library.HistoryLimit = 100;
        library.HistoryLimit = 1;
        Assert.Equal([c, pinned], Ids(library));
    }

    [Fact]
    public void ClearHistoryKeepsPinnedClips()
    {
        var library = Open();
        var board = library.AddPinboard("Work");
        var pinned = Insert(library, "pinned");
        library.SetPinboard(board, new HashSet<Guid> { pinned });
        var loose = Insert(library, "loose");

        library.ClearHistory();

        Assert.Equal([pinned], Ids(library));
        library.Flush();
        Assert.False(Storage.HasPayload(loose));
        Assert.True(Storage.HasPayload(pinned));
    }

    [Fact]
    public void DeletePinboardReturnsClipsToHistoryAndTrims()
    {
        var library = Open(historyLimit: 2);
        var board = library.AddPinboard("Work");
        var a = Insert(library, "a");
        var b = Insert(library, "b");
        library.SetPinboard(board, new HashSet<Guid> { a, b });
        var c = Insert(library, "c");
        var d = Insert(library, "d");
        Assert.Equal([d, c, b, a], Ids(library));

        library.DeletePinboard(board);

        Assert.Empty(library.Pinboards);
        Assert.Equal([d, c], Ids(library));
        Assert.All(library.Clips, clip => Assert.Null(clip.PinboardId));
        library.Flush();
        Assert.False(Storage.HasPayload(a));
        Assert.False(Storage.HasPayload(b));
    }

    [Fact]
    public void AddPinboardTakesTheFirstUnusedColorThenCycles()
    {
        var library = Open();
        var first = library.AddPinboard("1");
        library.AddPinboard("2");
        library.SetPinboardColor(first, PinboardColor.Yellow);
        library.AddPinboard("3");
        Assert.Equal([PinboardColor.Yellow, PinboardColor.Orange, PinboardColor.Red], library.Pinboards.Select(p => p.Color));

        for (var index = 3; index < 8; index++) library.AddPinboard($"{index + 1}");
        Assert.Equal(Enum.GetValues<PinboardColor>().Order(), library.Pinboards.Select(p => p.Color).Order());
        library.AddPinboard("9");
        Assert.Equal(PinboardColor.Red, library.Pinboards[8].Color);
    }

    [Fact]
    public void FlushWritesTheIndexAndANewLibraryReloadsIt()
    {
        var library = Open();
        var board = library.AddPinboard("Work");
        var kept = Insert(library, "kept");
        library.SetPinboard(board, new HashSet<Guid> { kept });
        library.Rename(kept, "Named");
        var other = Insert(library, "other");
        library.Flush();
        Assert.False(library.HasUnsavedChanges);
        Assert.True(File.Exists(_dir.File("index.json")));

        var reopened = Open();
        Assert.Equal([other, kept], Ids(reopened));
        Assert.Equal(library.Clips, reopened.Clips);
        Assert.Equal(library.Pinboards, reopened.Pinboards);
        Assert.Equal("kept", reopened.Payload(kept)!.Text);
    }

    [Fact]
    public void LoadDropsClipsWithoutPayloadAndRemovesOrphans()
    {
        var library = Open();
        var kept = Insert(library, "kept");
        var lost = Insert(library, "lost");
        library.Flush();
        File.Delete(_dir.File("payloads", $"{lost:D}.bin"));
        var orphanPayload = _dir.File("payloads", $"{Guid.NewGuid():D}.bin");
        var orphanThumbnail = _dir.File("thumbnails", "junk.png");
        Directory.CreateDirectory(_dir.File("thumbnails"));
        File.WriteAllBytes(orphanPayload, [1]);
        File.WriteAllBytes(orphanThumbnail, [1]);

        var reopened = Open();

        Assert.Equal([kept], Ids(reopened));
        Assert.True(Storage.HasPayload(kept));
        Assert.False(File.Exists(orphanPayload));
        Assert.False(File.Exists(orphanThumbnail));
    }

    [Fact]
    public void CorruptIndexIsMovedAsideAndTheLibraryOpensEmpty()
    {
        var library = Open();
        Insert(library, "x");
        library.Flush();
        File.WriteAllText(_dir.File("index.json"), "\0garbage");

        var reopened = Open();

        Assert.Empty(reopened.Clips);
        Assert.Empty(reopened.Pinboards);
        Assert.False(File.Exists(_dir.File("index.json")));
        var backup = Assert.Single(Directory.GetFiles(_dir.Path, "index.json.corrupt-*"));
        Assert.Equal("\0garbage", File.ReadAllText(backup));
    }

    [Theory]
    [InlineData("null")]
    [InlineData("""{ "clips": null, "pinboards": [] }""")]
    [InlineData("""{ "clips": [null], "pinboards": [] }""")]
    [InlineData("""{ "clips": [{ "id": "00000000-0000-0000-0000-000000000002", "createdAt": "2026-01-01T00:00:00Z", "content": { "kind": "text" }, "contentHash": "h", "byteCount": 1 }], "pinboards": [] }""")]
    [InlineData("""{ "clips": [], "pinboards": [{ "id": "00000000-0000-0000-0000-000000000001", "color": "Red" }] }""")]
    public void IncompleteIndexIsMovedAsideLikeACorruptOne(string json)
    {
        var library = Open();
        var kept = Insert(library, "x");
        library.Flush();
        File.WriteAllText(_dir.File("index.json"), json);

        var reopened = Open();

        Assert.Empty(reopened.Clips);
        Assert.Single(Directory.GetFiles(_dir.Path, "index.json.corrupt-*"));
        Assert.True(Storage.HasPayload(kept));
    }

    [Fact]
    public void PayloadsOfACorruptIndexStayOverLaterLaunches()
    {
        var library = Open();
        var old = Insert(library, "old");
        library.Flush();
        File.WriteAllText(_dir.File("index.json"), "\0garbage");

        var second = Open();
        var added = Insert(second, "new");
        second.Flush();
        var third = Open();

        Assert.Equal([added], Ids(third));
        Assert.True(Storage.HasPayload(old));
    }

    [Fact]
    public void RemovedClipFilesStayUntilTheIndexIsSaved()
    {
        var library = Open(historyLimit: 1);
        var oldest = Insert(library, "a");
        library.Flush();
        Insert(library, "b");
        Directory.CreateDirectory(_dir.File("index.json.blocked"));
        File.Move(_dir.File("index.json"), _dir.File("index.json.saved"));
        Directory.Move(_dir.File("index.json.blocked"), _dir.File("index.json"));

        var error = Record.Exception(library.Flush);
        Assert.True(error is IOException or UnauthorizedAccessException, error?.ToString());
        Assert.True(Storage.HasPayload(oldest));

        Directory.Delete(_dir.File("index.json"));
        library.Flush();
        Assert.False(Storage.HasPayload(oldest));
    }
}
