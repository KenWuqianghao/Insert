using System.Buffers.Binary;
using System.Text;

namespace Insert.Core.Tests;

public sealed class CaptureTests
{
    private static Representation Utf16(string format, string text) => new(format, Encoding.Unicode.GetBytes(text + "\0"));

    private static Representation Utf8(string format, string text) => new(format, Encoding.UTF8.GetBytes(text));

    private static ClipContent? Classify(params Representation[] representations) =>
        ClipCapture.Classify(new ClipPayload(representations), new FakeImageCodec())?.Content;

    private static string CfHtml(string fragment) =>
        "Version:0.9\r\nStartHTML:0000000105\r\nEndHTML:0000000200\r\nStartFragment:0000000141\r\nEndFragment:0000000180\r\n"
        + $"<html><body>\r\n<!--StartFragment-->{fragment}<!--EndFragment-->\r\n</body></html>";

    [Fact]
    public void BuildMakesAClipFromThePayload()
    {
        var payload = ClipPayload.FromText("hello");
        var source = new SourceApp(@"C:\notepad.exe", "Notepad");

        var capture = ClipCapture.Build(payload, source, new FakeImageCodec(), Fixtures.Now);

        Assert.NotNull(capture);
        Assert.Equal(new TextContent(TextSnippet.From("hello")), capture.Clip.Content);
        Assert.Equal(source, capture.Clip.Source);
        Assert.Equal(Fixtures.Now, capture.Clip.CreatedAt);
        Assert.Equal(payload.ContentHash, capture.Clip.ContentHash);
        Assert.Equal(10, capture.Clip.ByteCount);
        Assert.Null(capture.Clip.PinboardId);
        Assert.Null(capture.Thumbnail);
    }

    [Fact]
    public void PlainTextIsText()
    {
        Assert.Equal(new TextContent(TextSnippet.From("  hello\nworld ")), Classify(Utf16(ClipFormats.UnicodeText, "  hello\nworld ")));
    }

    [Fact]
    public void WebAddressTextIsALink()
    {
        Assert.Equal(new LinkContent("https://example.com/a?b=c"), Classify(Utf16(ClipFormats.UnicodeText, "  https://example.com/a?b=c\n")));
        Assert.IsType<TextContent>(Classify(Utf16(ClipFormats.UnicodeText, "ftp://example.com")));
        Assert.IsType<TextContent>(Classify(Utf16(ClipFormats.UnicodeText, "https://example.com and more")));
    }

    [Fact]
    public void UrlFormatIsALink()
    {
        var content = Classify(Utf16(ClipFormats.Url, " https://example.com/page "), Utf16(ClipFormats.UnicodeText, "some other text"));
        Assert.Equal(new LinkContent("https://example.com/page"), content);
    }

    [Fact]
    public void LinkTitleComesFromTheHtmlAnchor()
    {
        var html = Utf8(ClipFormats.Html, CfHtml("<a class=\"x\" href=\"https://example.com/?a=1&amp;b=2\"><b>Example</b> &amp; Co</a>"));

        Assert.Equal(new LinkContent("https://example.com/?a=1&b=2", "Example & Co"), Classify(Utf16(ClipFormats.Url, "https://example.com/?a=1&b=2"), html));
        Assert.Equal(new LinkContent("https://example.com/?a=1&b=2", "Example & Co"), Classify(Utf16(ClipFormats.UnicodeText, "https://example.com/?a=1&b=2"), html));
        Assert.Equal(new LinkContent("https://other.com"), Classify(Utf16(ClipFormats.Url, "https://other.com"), html));
    }

    [Fact]
    public void HexColorTextIsAColor()
    {
        Assert.Equal(new ColorContent(ColorValue.FromHex("#FF8800")!.Value), Classify(Utf16(ClipFormats.UnicodeText, " #ff8800 ")));
        Assert.IsType<TextContent>(Classify(Utf16(ClipFormats.UnicodeText, "#ff88")));
    }

    [Fact]
    public void RtfWithTextIsRichTextOfThatText()
    {
        var content = Classify(Utf16(ClipFormats.UnicodeText, "plain words"), Utf8(ClipFormats.Rtf, @"{\rtf1\ansi other}"));
        Assert.Equal(new RichTextContent(TextSnippet.From("plain words")), content);
    }

    [Fact]
    public void RtfWithoutTextIsRichTextOfTheRtfText()
    {
        const string rtf = @"{\rtf1\ansi{\fonttbl\f0\fswiss Helvetica;}{\colortbl;\red255\green0\blue0;}{\*\generator Word;}\f0 Hello \b bold\b0\par Caf\'e9 \u8364?\{x\}}";

        Assert.Equal("Hello bold\nCafé €{x}", ClipCapture.RtfText(Encoding.Latin1.GetBytes(rtf)));
        Assert.Equal(new RichTextContent(TextSnippet.From("Hello bold\nCafé €{x}")), Classify(Utf8(ClipFormats.Rtf, rtf)));
        Assert.Null(Classify(Utf8(ClipFormats.Rtf, @"{\rtf1{\fonttbl\f0 Arial;}}")));
    }

    [Theory]
    [InlineData(@"{\rtf1\ansi\ansicpg1252 \'80}", "€")]
    [InlineData(@"{\rtf1\ansi\ansicpg936 \'c4\'e3\'ba\'c3}", "你好")]
    [InlineData(@"{\rtf1\ansi\uc0\u945X}", "αX")]
    [InlineData(@"{\rtf1\ansi\uc2\u945\'80\'80X}", "αX")]
    [InlineData(@"{\rtf1\ansi{\uc0 \u945}\u946?X}", "αβX")]
    public void RtfTextUsesTheCodePageAndTheUnicodeFallbackCount(string rtf, string text) =>
        Assert.Equal(text, ClipCapture.RtfText(Encoding.Latin1.GetBytes(rtf)));

    [Fact]
    public void HtmlOnlyIsStrippedRichText()
    {
        var content = Classify(Utf8(ClipFormats.Html, CfHtml("<p>Hello\n <b>world</b> &amp; more</p>")));
        Assert.Equal(new RichTextContent(TextSnippet.From("Hello world & more")), content);
        Assert.Null(Classify(Utf8(ClipFormats.Html, CfHtml("<img src=\"x.png\">"))));
    }

    [Fact]
    public void WhitespaceOnlyTextGivesNoClip()
    {
        var payload = new ClipPayload([Utf16(ClipFormats.UnicodeText, " \r\n\t ")]);
        Assert.Null(ClipCapture.Build(payload, null, new FakeImageCodec(), Fixtures.Now));
    }

    [Fact]
    public void FileDropIsFiles()
    {
        var content = Classify(Utf8(ClipFormats.FileDrop, "C:\\a.txt\nC:\\dir\\b.png\n"), Utf16(ClipFormats.UnicodeText, "text"));
        var files = Assert.IsType<FilesContent>(content);
        Assert.Equal([@"C:\a.txt", @"C:\dir\b.png"], files.Paths);
    }

    [Fact]
    public void ImageUsesTheCodecForSizeAndThumbnail()
    {
        var codec = new FakeImageCodec((new PixelSize(640, 480), [7, 7]));

        var capture = ClipCapture.Build(new ClipPayload([new Representation(ClipFormats.Png, [1, 2, 3])]), null, codec, Fixtures.Now);

        Assert.Equal(new ImageContent(new PixelSize(640, 480)), capture?.Clip.Content);
        Assert.Equal([7, 7], capture?.Thumbnail);
        Assert.Equal([1, 2, 3], codec.LastInput);
    }

    [Fact]
    public void DibImageReachesTheCodecAsBmp()
    {
        var codec = new FakeImageCodec((new PixelSize(2, 2), [7]));
        var dib = DibHeader(40, 32, 0);

        var content = ClipCapture.Classify(new ClipPayload([new Representation(ClipFormats.Dib, dib)]), codec)?.Content;

        Assert.IsType<ImageContent>(content);
        Assert.Equal(Dib.ToBmp(dib), codec.LastInput);
    }

    [Fact]
    public void ImageWithTextIsText()
    {
        var codec = new FakeImageCodec((new PixelSize(640, 480), [7, 7]));
        var payload = new ClipPayload([new Representation(ClipFormats.Png, [1]), Utf16(ClipFormats.UnicodeText, "cell")]);

        Assert.Equal(new TextContent(TextSnippet.From("cell")), ClipCapture.Classify(payload, codec)?.Content);
    }

    [Fact]
    public void ImageThatTheCodecCannotReadGivesNoClip()
    {
        Assert.Null(ClipCapture.Classify(new ClipPayload([new Representation(ClipFormats.Png, [1])]), new FakeImageCodec()));
    }

    [Fact]
    public void PayloadOverTheSizeLimitGivesNoClip()
    {
        var payload = new ClipPayload([
            Utf16(ClipFormats.UnicodeText, "text"),
            new Representation("Big", new byte[ClipCapture.MaxByteCount]),
        ]);
        Assert.Null(ClipCapture.Build(payload, null, new FakeImageCodec(), Fixtures.Now));
        Assert.Null(ClipCapture.Build(new ClipPayload([]), null, new FakeImageCodec(), Fixtures.Now));
    }

    [Fact]
    public void PrivacyMarkersMakeACopyPrivate()
    {
        static byte[] Dword(int value)
        {
            var bytes = new byte[4];
            BinaryPrimitives.WriteInt32LittleEndian(bytes, value);
            return bytes;
        }

        Assert.True(ClipCapture.IsPrivate([ClipFormats.UnicodeText, ClipFormats.ExcludeFromMonitor], _ => null));
        Assert.True(ClipCapture.IsPrivate([ClipFormats.ViewerIgnore], _ => null));
        Assert.True(ClipCapture.IsPrivate([ClipFormats.CanIncludeInHistory], _ => Dword(0)));
        Assert.False(ClipCapture.IsPrivate([ClipFormats.CanIncludeInHistory], _ => Dword(1)));
        Assert.False(ClipCapture.IsPrivate([ClipFormats.CanIncludeInHistory], _ => null));
        Assert.False(ClipCapture.IsPrivate([ClipFormats.UnicodeText], _ => throw new InvalidOperationException("must not read")));
    }

    private static byte[] DibHeader(int headerSize, ushort bitCount, int compression, int colorsUsed = 0, int pixelBytes = 16)
    {
        var dib = new byte[headerSize + pixelBytes];
        BinaryPrimitives.WriteInt32LittleEndian(dib, headerSize);
        BinaryPrimitives.WriteInt32LittleEndian(dib.AsSpan(4), 2);
        BinaryPrimitives.WriteInt32LittleEndian(dib.AsSpan(8), 2);
        BinaryPrimitives.WriteUInt16LittleEndian(dib.AsSpan(12), 1);
        BinaryPrimitives.WriteUInt16LittleEndian(dib.AsSpan(14), bitCount);
        BinaryPrimitives.WriteInt32LittleEndian(dib.AsSpan(16), compression);
        BinaryPrimitives.WriteInt32LittleEndian(dib.AsSpan(32), colorsUsed);
        return dib;
    }

    [Theory]
    [InlineData(40, 32, 0, 0, 54)]
    [InlineData(40, 32, 3, 0, 66)]
    [InlineData(124, 32, 3, 0, 138)]
    [InlineData(40, 8, 0, 0, 54 + 256 * 4)]
    [InlineData(40, 8, 0, 16, 54 + 16 * 4)]
    public void DibToBmpWritesTheFileHeader(int headerSize, int bitCount, int compression, int colorsUsed, int pixelOffset)
    {
        var dib = DibHeader(headerSize, (ushort)bitCount, compression, colorsUsed);

        var bmp = Dib.ToBmp(dib)!;

        Assert.Equal("BM"u8.ToArray(), bmp[..2]);
        Assert.Equal(bmp.Length, BinaryPrimitives.ReadInt32LittleEndian(bmp.AsSpan(2)));
        Assert.Equal(pixelOffset, BinaryPrimitives.ReadInt32LittleEndian(bmp.AsSpan(10)));
        Assert.Equal(dib, bmp[14..]);
        Assert.Equal(dib, Dib.FromBmp(bmp));
    }

    [Fact]
    public void DibToBmpRejectsAShortHeader()
    {
        Assert.Null(Dib.ToBmp(new byte[39]));
        var coreHeader = DibHeader(40, 32, 0);
        BinaryPrimitives.WriteInt32LittleEndian(coreHeader, 12);
        Assert.Null(Dib.ToBmp(coreHeader));
    }

    [Fact]
    public void PlainTextFallsBackFromTextToFilesToUrlToRichText()
    {
        var text = Utf16(ClipFormats.UnicodeText, "text");
        var files = Utf8(ClipFormats.FileDrop, "C:\\a\nC:\\b");
        var url = Utf16(ClipFormats.Url, "https://example.com");

        Assert.Equal("text", new ClipPayload([url, files, text]).PlainText);
        Assert.Equal("C:\\a\r\nC:\\b", new ClipPayload([url, files]).PlainText);
        Assert.Equal("https://example.com", new ClipPayload([url]).PlainText);
        var rtf = Utf8(ClipFormats.Rtf, @"{\rtf1\ansi {\b Bold} text}");
        var html = Utf8(ClipFormats.Html, "<b>Hello</b>");
        Assert.Equal("https://example.com", new ClipPayload([rtf, url]).PlainText);
        Assert.Equal("Bold text", new ClipPayload([html, rtf]).PlainText);
        Assert.Equal("Hello", new ClipPayload([html]).PlainText);
        Assert.Null(new ClipPayload([Utf8(ClipFormats.Png, "x")]).PlainText);
    }
}
