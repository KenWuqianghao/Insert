using System.Globalization;

namespace Insert.Core.Tests;

public sealed class ModelTests : IDisposable
{
    private readonly TempDirectory _dir = new();

    public void Dispose() => _dir.Dispose();

    [Theory]
    [InlineData("#f80", 1.0, 0x88 / 255.0, 0.0, 1.0)]
    [InlineData("#FF8800", 1.0, 0x88 / 255.0, 0.0, 1.0)]
    [InlineData("#00000080", 0.0, 0.0, 0.0, 0x80 / 255.0)]
    public void ColorFromHexReadsThreeSixAndEightDigits(string hex, double red, double green, double blue, double alpha)
    {
        Assert.Equal(new ColorValue(red, green, blue, alpha), ColorValue.FromHex(hex));
    }

    [Theory]
    [InlineData("FF8800")]
    [InlineData("#")]
    [InlineData("#ff88")]
    [InlineData("#ff880")]
    [InlineData("#ff88zz")]
    [InlineData("#ff8800ff00")]
    public void ColorFromHexRejectsInvalidText(string text)
    {
        Assert.Null(ColorValue.FromHex(text));
    }

    [Theory]
    [InlineData("#FF8800")]
    [InlineData("#12345680")]
    public void ColorHexRoundTrips(string hex)
    {
        Assert.Equal(hex, ColorValue.FromHex(hex)!.Value.Hex);
    }

    [Fact]
    public void ColorIsLightUsesLuminance()
    {
        Assert.True(ColorValue.FromHex("#FFFFFF")!.Value.IsLight);
        Assert.True(ColorValue.FromHex("#FFFF00")!.Value.IsLight);
        Assert.False(ColorValue.FromHex("#0000FF")!.Value.IsLight);
        Assert.False(ColorValue.FromHex("#000")!.Value.IsLight);
    }

    [Fact]
    public void TextSnippetLimitCountsGraphemeClusters()
    {
        var family = "👨‍👩‍👧";
        var text = string.Concat(Enumerable.Repeat(family, TextSnippet.Limit + 5));

        var snippet = TextSnippet.From(text);

        Assert.Equal(TextSnippet.Limit + 5, snippet.CharacterCount);
        Assert.Equal(TextSnippet.Limit, new StringInfo(snippet.Text).LengthInTextElements);
        Assert.Equal(TextSnippet.Limit * family.Length, snippet.Text.Length);
        Assert.Equal("short", TextSnippet.From("short").Text);
    }

    [Fact]
    public void FirstLineSkipsBlankLinesAndTrims()
    {
        Assert.Equal("first", TextSnippet.From("\r\n  \n\t first \r\nsecond").FirstLine);
        Assert.Null(TextSnippet.From(" \n\t\n").FirstLine);
    }

    [Fact]
    public void DisplayTitleAndFooterOfEachContentKind()
    {
        (ClipContent Content, string Title, string Footer)[] cases =
        [
            (new TextContent(TextSnippet.From("\nHello\nworld")), "Hello", "12 characters"),
            (new TextContent(TextSnippet.From("  ")), "Text", "2 characters"),
            (new RichTextContent(TextSnippet.From("x")), "x", "1 character"),
            (new RichTextContent(TextSnippet.From("")), "Rich Text", "0 characters"),
            (new LinkContent("https://example.com/a", "Example"), "Example", "21 characters"),
            (new LinkContent("https://example.com/a"), "example.com", "21 characters"),
            (new LinkContent("not a url"), "not a url", "9 characters"),
            (new ImageContent(new PixelSize(1920, 1080)), "Image", "1,920 × 1,080"),
            (new FilesContent([@"C:\Users\me\report.pdf"]), "report.pdf", "1 file"),
            (new FilesContent([@"C:\a.txt", "/home/b.txt"]), "2 Files", "2 files"),
            (new ColorContent(ColorValue.FromHex("#FF8800")!.Value), "#FF8800", "RGB 255, 136, 0"),
            (new PdfContent(), "PDF Document", "1.5 KB"),
            (new MediaContent(), "Media", "1.5 KB"),
            (new DataContent(), "Clipboard Data", "1.5 KB"),
        ];

        foreach (var (content, title, footer) in cases)
        {
            Assert.Equal(title, content.DisplayTitle);
            Assert.Equal(footer, content.Footer(1_500));
        }
    }

    [Theory]
    [InlineData(0, "Just now")]
    [InlineData(-30, "Just now")]
    [InlineData(59, "Just now")]
    [InlineData(60, "1 min ago")]
    [InlineData(3_599, "59 min ago")]
    [InlineData(3_600, "1 hr ago")]
    [InlineData(86_399, "23 hr ago")]
    [InlineData(86_400, "1 day ago")]
    [InlineData(2 * 86_400, "2 days ago")]
    [InlineData(7 * 86_400 - 1, "6 days ago")]
    [InlineData(7 * 86_400, "1 week ago")]
    [InlineData(14 * 86_400, "2 weeks ago")]
    [InlineData(35 * 86_400 - 1, "4 weeks ago")]
    [InlineData(35 * 86_400, "1 mo ago")]
    [InlineData(360 * 86_400 - 1, "11 mo ago")]
    [InlineData(360 * 86_400, "0 yr ago")]
    [InlineData(365 * 86_400, "1 yr ago")]
    [InlineData(800 * 86_400, "2 yr ago")]
    public void RelativeTimeBoundaries(long secondsAgo, string expected)
    {
        Assert.Equal(expected, Format.RelativeTime(Fixtures.Now.AddSeconds(-secondsAgo), Fixtures.Now));
    }

    [Fact]
    public void MatchesIsCaseInsensitiveOverContentTitleAndSourceName()
    {
        var clip = Fixtures.Clip(new TextContent(TextSnippet.From("Body Text")), title: "My Title", source: new SourceApp(@"C:\x\notepad.exe", "Notepad"));

        Assert.True(clip.Matches("body text"));
        Assert.True(clip.Matches("MY TITLE"));
        Assert.True(clip.Matches("notepad"));
        Assert.True(clip.Matches("TEXT"));
        Assert.False(clip.Matches("x.exe"));
        Assert.False(clip.Matches("missing"));
    }

    [Fact]
    public void IndexWithEveryContentKindRoundTripsThroughJson()
    {
        var board = new Pinboard(Guid.NewGuid(), "Work", PinboardColor.Teal);
        ClipContent[] contents =
        [
            new TextContent(TextSnippet.From("text")),
            new RichTextContent(TextSnippet.From("rich")),
            new LinkContent("https://example.com", "Example"),
            new LinkContent("https://example.org"),
            new ImageContent(new PixelSize(3, 4)),
            new FilesContent([@"C:\a.txt", @"C:\b.txt"]),
            new ColorContent(new ColorValue(0.25, 0.5, 0.75, 0.5)),
            new PdfContent(),
            new MediaContent(),
            new DataContent(),
        ];
        var clips = contents
            .Select((content, index) => Fixtures.Clip(content, index == 0 ? board.Id : null, index == 1 ? "Named" : null, index == 2 ? new SourceApp(@"C:\app.exe", "App") : null))
            .ToList();
        Assert.Equal(ClipKinds.All.Order(), clips.Select(clip => clip.Kind).Distinct().Order());
        var storage = new ClipStorage(_dir.Path);

        storage.SaveIndex(new LibraryIndex(clips, [board]));
        var loaded = storage.LoadIndex();

        Assert.Equal(clips, loaded.Clips);
        Assert.Equal([board], loaded.Pinboards);
    }
}
