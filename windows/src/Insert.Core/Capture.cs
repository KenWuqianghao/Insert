using System.Buffers.Binary;
using System.Security.Cryptography;
using System.Text;
using System.Text.RegularExpressions;

namespace Insert.Core;

/// <summary>
/// The names of the Windows clipboard formats that Insert stores. Only the clipboard code and this file use them.
/// Text formats hold their own encoding: UTF-16 for <see cref="UnicodeText"/> and <see cref="Url"/>, UTF-8 for the others.
/// </summary>
public static class ClipFormats
{
    public const string UnicodeText = "CF_UNICODETEXT";
    public const string Rtf = "Rich Text Format";
    public const string Html = "HTML Format";
    public const string Png = "PNG";
    public const string Dib = "CF_DIB";
    public const string DibV5 = "CF_DIBV5";
    /// <summary>Insert stores the paths of a file drop as UTF-8 text, one path on each line.</summary>
    public const string FileDrop = "CF_HDROP";
    public const string Url = "UniformResourceLocatorW";

    /// <summary>In the order that the clipboard reader asks for them.</summary>
    public static readonly IReadOnlyList<string> Stored = [UnicodeText, Rtf, Html, Png, DibV5, Dib, FileDrop, Url];

    /// <summary>Password managers and other apps set these formats on a copy that a clipboard history must not keep.</summary>
    public const string ExcludeFromMonitor = "ExcludeClipboardContentFromMonitorProcessing";
    public const string ViewerIgnore = "Clipboard Viewer Ignore";
    public const string CanIncludeInHistory = "CanIncludeInClipboardHistory";

    public static readonly IReadOnlyList<string> PrivacyMarkers = [ExcludeFromMonitor, ViewerIgnore, CanIncludeInHistory];
}

public sealed record Representation(string Format, byte[] Data);

/// <summary>The raw clipboard formats of one clip.</summary>
public sealed class ClipPayload(IReadOnlyList<Representation> representations)
{
    public IReadOnlyList<Representation> Representations { get; } = representations;

    public long ByteCount => Representations.Sum(representation => (long)representation.Data.Length);

    public string ContentHash
    {
        get
        {
            using var hash = IncrementalHash.CreateHash(HashAlgorithmName.SHA256);
            Span<byte> length = stackalloc byte[8];
            foreach (var representation in Representations)
            {
                hash.AppendData(Encoding.UTF8.GetBytes(representation.Format));
                BinaryPrimitives.WriteInt64LittleEndian(length, representation.Data.Length);
                hash.AppendData(length);
                hash.AppendData(representation.Data);
            }
            return Convert.ToHexStringLower(hash.GetHashAndReset());
        }
    }

    public byte[]? Get(string format) => Representations.FirstOrDefault(r => r.Format == format)?.Data;

    public bool Has(string format) => Get(format) is not null;

    public string? Text => Get(ClipFormats.UnicodeText) is { } data ? Encoding.Unicode.GetString(data).TrimEnd('\0') : null;

    public string? Url => Get(ClipFormats.Url) is { } data ? Encoding.Unicode.GetString(data).TrimEnd('\0').Trim() : null;

    public string? Html => Get(ClipFormats.Html) is { } data ? Encoding.UTF8.GetString(data).TrimEnd('\0') : null;

    public IReadOnlyList<string> FilePaths =>
        Get(ClipFormats.FileDrop) is { } data
            ? Encoding.UTF8.GetString(data).Split('\n', StringSplitOptions.RemoveEmptyEntries)
            : [];

    /// <summary>The text that "Paste as Plain Text" writes.</summary>
    public string? PlainText
    {
        get
        {
            if (Text is { } text) return text;
            var paths = FilePaths;
            if (paths.Count > 0) return string.Join("\r\n", paths);
            if (Url is { } url) return url;
            if (Get(ClipFormats.Rtf) is { } rtf) return ClipCapture.RtfText(rtf);
            return Html is { } html ? ClipCapture.HtmlText(html) : null;
        }
    }

    /// <summary>The image in a format that a common decoder reads: PNG, or a DIB with a BMP file header.</summary>
    public byte[]? ImageData =>
        Get(ClipFormats.Png) ?? ((Get(ClipFormats.DibV5) ?? Get(ClipFormats.Dib)) is { } dib ? Dib.ToBmp(dib) : null);

    public static ClipPayload FromText(string text) =>
        new([new Representation(ClipFormats.UnicodeText, Encoding.Unicode.GetBytes(text))]);
}

public sealed record CapturedClip(Clip Clip, ClipPayload Payload, byte[]? Thumbnail);

/// <summary>Reads image files. The app implements it with its image library.</summary>
public interface IImageCodec
{
    /// <summary>Returns the pixel size and a PNG thumbnail, or null when the data is not an image.</summary>
    (PixelSize Size, byte[] ThumbnailPng)? Inspect(byte[] encoded, int maxThumbnailSide);
}

public static partial class ClipCapture
{
    public const long MaxByteCount = 50L * 1024 * 1024;
    public const int ThumbnailSide = 560;

    /// <summary>True when an app marks the copy as private, for example a password from a password manager.</summary>
    public static bool IsPrivate(IReadOnlyCollection<string> formats, Func<string, byte[]?> read)
    {
        if (formats.Contains(ClipFormats.ExcludeFromMonitor) || formats.Contains(ClipFormats.ViewerIgnore)) return true;
        if (!formats.Contains(ClipFormats.CanIncludeInHistory)) return false;
        var value = read(ClipFormats.CanIncludeInHistory);
        return value is { Length: >= 4 } && BinaryPrimitives.ReadInt32LittleEndian(value) == 0;
    }

    public static CapturedClip? Build(ClipPayload payload, SourceApp? source, IImageCodec images, DateTimeOffset now)
    {
        if (payload.Representations.Count == 0 || payload.ByteCount > MaxByteCount) return null;
        if (Classify(payload, images) is not var (content, thumbnail)) return null;

        var clip = new Clip(Guid.NewGuid(), now, source, content, null, null, payload.ContentHash, payload.ByteCount);
        return new CapturedClip(clip, payload, thumbnail);
    }

    internal static (ClipContent Content, byte[]? Thumbnail)? Classify(ClipPayload payload, IImageCodec images)
    {
        var paths = payload.FilePaths;
        if (paths.Count > 0) return (new FilesContent(paths), null);

        var text = payload.Text;
        var hasText = !string.IsNullOrWhiteSpace(text);

        // Office and other apps add a picture of copied text or cells. Such a copy is text, so an image counts only without text.
        if (!hasText && payload.ImageData is { } imageData && images.Inspect(imageData, ThumbnailSide) is var (size, png))
            return (new ImageContent(size), png);

        if (payload.Url is { Length: > 0 } url) return (new LinkContent(url, AnchorTitle(payload.Html, url)), null);

        if (payload.Has(ClipFormats.Rtf))
        {
            var plain = hasText ? text : RtfText(payload.Get(ClipFormats.Rtf)!);
            return string.IsNullOrWhiteSpace(plain) ? null : (new RichTextContent(TextSnippet.From(plain)), null);
        }

        if (hasText)
        {
            var trimmed = text!.Trim();
            if (WebUrl(trimmed) is { } link) return (new LinkContent(link, AnchorTitle(payload.Html, link)), null);
            if (ColorValue.FromHex(trimmed) is { } color) return (new ColorContent(color), null);
            return (new TextContent(TextSnippet.From(text)), null);
        }

        if (payload.Html is { } html)
        {
            var stripped = HtmlText(html);
            return stripped.Length == 0 ? null : (new RichTextContent(TextSnippet.From(stripped)), null);
        }

        // Whitespace-only text reaches this point. It is not worth a card.
        return null;
    }

    /// <summary>An http or https address with a host and no whitespace.</summary>
    public static string? WebUrl(string text)
    {
        if (text.Any(char.IsWhiteSpace)) return null;
        if (!Uri.TryCreate(text, UriKind.Absolute, out var uri)) return null;
        if (uri.Scheme != Uri.UriSchemeHttp && uri.Scheme != Uri.UriSchemeHttps) return null;
        return uri.Host.Length > 0 ? text : null;
    }

    /// <summary>A browser that copies a link as HTML puts the page title in the anchor.</summary>
    internal static string? AnchorTitle(string? html, string url)
    {
        if (html is null) return null;
        var fragment = HtmlFragment(html);
        var match = AnchorPattern().Match(fragment);
        if (!match.Success || System.Net.WebUtility.HtmlDecode(match.Groups["href"].Value) != url) return null;
        var title = HtmlText(match.Groups["body"].Value);
        return title.Length == 0 || title == url ? null : title;
    }

    /// <summary>The CF_HTML format puts a header with byte offsets before the HTML.</summary>
    internal static string HtmlFragment(string html)
    {
        const string start = "<!--StartFragment-->";
        const string end = "<!--EndFragment-->";
        var from = html.IndexOf(start, StringComparison.OrdinalIgnoreCase);
        var to = html.IndexOf(end, StringComparison.OrdinalIgnoreCase);
        if (from >= 0 && to > from) return html[(from + start.Length)..to];
        var tag = html.IndexOf('<');
        return tag >= 0 ? html[tag..] : html;
    }

    internal static string HtmlText(string html)
    {
        var fragment = HtmlFragment(html);
        var noTags = TagPattern().Replace(fragment, " ");
        return SpacePattern().Replace(System.Net.WebUtility.HtmlDecode(noTags), " ").Trim();
    }

    /// <summary>A small RTF reader for the plain text of a copy that has RTF and no text.</summary>
    internal static string RtfText(byte[] rtf)
    {
        var source = Encoding.Latin1.GetString(rtf);
        var output = new StringBuilder();
        var bytes = new List<byte>();
        var encoding = CodePage(1252);
        var groups = new Stack<(int Uc, bool Skip)>();
        var (uc, skip) = (1, false);
        // The characters after \uN that are the fallback for readers without Unicode. A control or an \'hh is one character.
        var fallback = 0;

        void FlushBytes()
        {
            if (bytes.Count == 0) return;
            output.Append(encoding.GetString(bytes.ToArray()));
            bytes.Clear();
        }

        for (var index = 0; index < source.Length; index++)
        {
            var c = source[index];
            if (c is '{' or '}')
            {
                FlushBytes();
                fallback = 0;
                if (c == '{')
                {
                    groups.Push((uc, skip));
                    if (index + 2 < source.Length && source[index + 1] == '\\' && source[index + 2] == '*') skip = true;
                }
                else if (groups.Count > 0)
                {
                    (uc, skip) = groups.Pop();
                }
                continue;
            }
            if (c is '\r' or '\n') continue;
            if (c != '\\' || index + 1 >= source.Length)
            {
                FlushBytes();
                if (fallback > 0) fallback--;
                else if (!skip) output.Append(c);
                continue;
            }

            var next = source[index + 1];
            if (next == '\'')
            {
                if (index + 3 < source.Length && byte.TryParse(source.AsSpan(index + 2, 2), System.Globalization.NumberStyles.HexNumber, null, out var value))
                {
                    if (fallback > 0) fallback--;
                    else if (!skip) bytes.Add(value);
                }
                index += 3;
                continue;
            }
            FlushBytes();
            if (!char.IsAsciiLetter(next))
            {
                index++;
                if (fallback > 0) fallback--;
                else if (!skip && next is '\\' or '{' or '}') output.Append(next);
                else if (!skip && next == '~') output.Append(' ');
                continue;
            }

            var end = index + 1;
            while (end < source.Length && char.IsAsciiLetter(source[end])) end++;
            var word = source[(index + 1)..end];
            var numberStart = end;
            if (end < source.Length && (source[end] == '-' || char.IsAsciiDigit(source[end]))) end++;
            while (end < source.Length && char.IsAsciiDigit(source[end])) end++;
            var number = int.TryParse(source.AsSpan(numberStart, end - numberStart), out var parsed) ? parsed : (int?)null;
            if (end < source.Length && source[end] == ' ') end++;
            index = end - 1;

            if (fallback > 0)
            {
                fallback--;
                continue;
            }
            if (SkippedGroups.Contains(word)) skip = true;
            if (skip) continue;
            switch (word)
            {
                case "ansicpg" when number is { } page:
                    encoding = CodePage(page);
                    break;
                case "uc" when number is { } count:
                    uc = Math.Max(count, 0);
                    break;
                case "u" when number is { } code:
                    output.Append((char)(short)code);
                    fallback = uc;
                    break;
                case "par" or "line" or "row":
                    output.Append('\n');
                    break;
                case "tab" or "cell":
                    output.Append('\t');
                    break;
            }
        }
        FlushBytes();
        return output.ToString().Trim();
    }

    private static readonly HashSet<string> SkippedGroups = ["fonttbl", "colortbl", "stylesheet", "info", "pict", "header", "footer", "listtable", "listoverridetable", "rsidtbl", "generator", "themedata", "datastore", "latentstyles"];

    private static Encoding CodePage(int page)
    {
        try
        {
            return CodePagesEncodingProvider.Instance.GetEncoding(page) ?? Encoding.GetEncoding(page);
        }
        catch (Exception error) when (error is ArgumentException or NotSupportedException)
        {
            return Encoding.Latin1;
        }
    }

    [GeneratedRegex("<a\\s[^>]*href\\s*=\\s*[\"'](?<href>[^\"']*)[\"'][^>]*>(?<body>.*?)</a>", RegexOptions.IgnoreCase | RegexOptions.Singleline)]
    private static partial Regex AnchorPattern();

    [GeneratedRegex("<[^>]+>")]
    private static partial Regex TagPattern();

    [GeneratedRegex("\\s+")]
    private static partial Regex SpacePattern();
}

/// <summary>A device-independent bitmap is a BMP file without the 14-byte file header.</summary>
public static class Dib
{
    public static byte[]? ToBmp(byte[] dib)
    {
        if (dib.Length < 40) return null;
        var headerSize = BinaryPrimitives.ReadInt32LittleEndian(dib);
        if (headerSize < 40 || headerSize > dib.Length) return null;
        var bitCount = BinaryPrimitives.ReadUInt16LittleEndian(dib.AsSpan(14));
        var compression = BinaryPrimitives.ReadInt32LittleEndian(dib.AsSpan(16));
        var colorsUsed = BinaryPrimitives.ReadInt32LittleEndian(dib.AsSpan(32));

        const int biBitfields = 3;
        var masks = headerSize == 40 && compression == biBitfields ? 12 : 0;
        var colors = colorsUsed > 0 ? colorsUsed : bitCount <= 8 ? 1 << bitCount : 0;
        var offset = 14 + headerSize + masks + colors * 4;

        var bmp = new byte[14 + dib.Length];
        bmp[0] = (byte)'B';
        bmp[1] = (byte)'M';
        BinaryPrimitives.WriteInt32LittleEndian(bmp.AsSpan(2), bmp.Length);
        BinaryPrimitives.WriteInt32LittleEndian(bmp.AsSpan(10), offset);
        dib.CopyTo(bmp, 14);
        return bmp;
    }

    /// <summary>The DIB of a BMP file, for apps that read only CF_DIB.</summary>
    public static byte[]? FromBmp(byte[] bmp) =>
        bmp.Length > 54 && bmp[0] == 'B' && bmp[1] == 'M' ? bmp[14..] : null;
}
