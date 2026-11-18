using System.Globalization;
using System.Text.Json.Serialization;

namespace Insert.Core;

/// <summary>The app that was in front when the clip was copied.</summary>
/// <param name="Id">The full path of the executable. Compare ids with <see cref="SameId"/>.</param>
public sealed record SourceApp(string Id, string Name)
{
    public bool SameId(string other) => string.Equals(Id, other, StringComparison.OrdinalIgnoreCase);
}

public readonly record struct PixelSize(int Width, int Height);

public readonly record struct ColorValue(double Red, double Green, double Blue, double Alpha)
{
    /// <summary>Accepts <c>#RGB</c>, <c>#RRGGBB</c>, and <c>#RRGGBBAA</c>.</summary>
    public static ColorValue? FromHex(string text)
    {
        if (!text.StartsWith('#')) return null;
        var digits = text[1..];
        if (digits.Length == 0 || !digits.All(Uri.IsHexDigit)) return null;
        digits = digits.Length switch
        {
            3 => string.Concat(digits.Select(c => new string(c, 2))) + "FF",
            6 => digits + "FF",
            8 => digits,
            _ => null,
        } ?? "";
        if (digits.Length != 8) return null;

        var rgba = uint.Parse(digits, NumberStyles.HexNumber, CultureInfo.InvariantCulture);
        double Channel(int shift) => ((rgba >> shift) & 0xFF) / 255.0;
        return new ColorValue(Channel(24), Channel(16), Channel(8), Channel(0));
    }

    [JsonIgnore]
    public (byte R, byte G, byte B) Rgb8 => (ToByte(Red), ToByte(Green), ToByte(Blue));

    /// <summary><c>#RRGGBB</c>, plus <c>AA</c> when the color is not opaque.</summary>
    [JsonIgnore]
    public string Hex
    {
        get
        {
            var (r, g, b) = Rgb8;
            var rgb = $"#{r:X2}{g:X2}{b:X2}";
            return Alpha < 0.999 ? rgb + ToByte(Alpha).ToString("X2", CultureInfo.InvariantCulture) : rgb;
        }
    }

    [JsonIgnore]
    public bool IsLight => 0.299 * Red + 0.587 * Green + 0.114 * Blue > 0.62;

    private static byte ToByte(double channel) => (byte)Math.Round(Math.Clamp(channel, 0, 1) * 255);
}

/// <summary>The index keeps only the start of a long text. The payload file keeps all of it.</summary>
public sealed record TextSnippet(string Text, int CharacterCount)
{
    public const int Limit = 8_000;

    public static TextSnippet From(string fullText)
    {
        var count = 0;
        var end = fullText.Length;
        var enumerator = StringInfo.GetTextElementEnumerator(fullText);
        while (enumerator.MoveNext())
        {
            if (count == Limit) end = enumerator.ElementIndex;
            count++;
        }
        return new TextSnippet(fullText[..end], count);
    }

    [JsonIgnore]
    public string? FirstLine =>
        Text.Split(['\n', '\r', '\v', '\f', '\u0085', '\u2028', '\u2029'])
            .Select(line => line.Trim())
            .FirstOrDefault(line => line.Length > 0);
}

public enum ClipKind
{
    Text,
    RichText,
    Link,
    Image,
    Files,
    Color,
    Pdf,
    Media,
    Data,
}

public static class ClipKinds
{
    public static readonly IReadOnlyList<ClipKind> All = Enum.GetValues<ClipKind>();

    public static string Name(this ClipKind kind) => kind switch
    {
        ClipKind.Text => "Text",
        ClipKind.RichText => "Rich Text",
        ClipKind.Link => "Link",
        ClipKind.Image => "Image",
        ClipKind.Files => "Files",
        ClipKind.Color => "Color",
        ClipKind.Pdf => "PDF",
        ClipKind.Media => "Media",
        ClipKind.Data => "Data",
        _ => throw new ArgumentOutOfRangeException(nameof(kind)),
    };
}

[JsonPolymorphic(TypeDiscriminatorPropertyName = "kind")]
[JsonDerivedType(typeof(TextContent), "text")]
[JsonDerivedType(typeof(RichTextContent), "richText")]
[JsonDerivedType(typeof(LinkContent), "link")]
[JsonDerivedType(typeof(ImageContent), "image")]
[JsonDerivedType(typeof(FilesContent), "files")]
[JsonDerivedType(typeof(ColorContent), "color")]
[JsonDerivedType(typeof(PdfContent), "pdf")]
[JsonDerivedType(typeof(MediaContent), "media")]
[JsonDerivedType(typeof(DataContent), "data")]
public abstract record ClipContent
{
    [JsonIgnore]
    public abstract ClipKind Kind { get; }

    [JsonIgnore]
    public abstract string DisplayTitle { get; }

    [JsonIgnore]
    public abstract string SearchText { get; }

    public abstract string Footer(long byteCount);
}

public sealed record TextContent(TextSnippet Snippet) : ClipContent
{
    [JsonIgnore]
    public override ClipKind Kind => ClipKind.Text;
    [JsonIgnore]
    public override string DisplayTitle => Snippet.FirstLine ?? Kind.Name();
    [JsonIgnore]
    public override string SearchText => Snippet.Text;
    public override string Footer(long byteCount) => Format.Counted(Snippet.CharacterCount, "character");
}

public sealed record RichTextContent(TextSnippet Snippet) : ClipContent
{
    [JsonIgnore]
    public override ClipKind Kind => ClipKind.RichText;
    [JsonIgnore]
    public override string DisplayTitle => Snippet.FirstLine ?? Kind.Name();
    [JsonIgnore]
    public override string SearchText => Snippet.Text;
    public override string Footer(long byteCount) => Format.Counted(Snippet.CharacterCount, "character");
}

public sealed record LinkContent(string Url, string? Title = null) : ClipContent
{
    [JsonIgnore]
    public override ClipKind Kind => ClipKind.Link;
    [JsonIgnore]
    public override string DisplayTitle => Title ?? Host ?? Url;
    [JsonIgnore]
    public override string SearchText => Title is null ? Url : $"{Title} {Url}";
    public override string Footer(long byteCount) => Format.Counted(new StringInfo(Url).LengthInTextElements, "character");

    [JsonIgnore]
    public string? Host => Uri.TryCreate(Url, UriKind.Absolute, out var uri) && uri.Host.Length > 0 ? uri.Host : null;
}

public sealed record ImageContent(PixelSize Size) : ClipContent
{
    [JsonIgnore]
    public override ClipKind Kind => ClipKind.Image;
    [JsonIgnore]
    public override string DisplayTitle => "Image";
    [JsonIgnore]
    public override string SearchText => Kind.Name();
    public override string Footer(long byteCount) =>
        $"{Size.Width.ToString("N0", CultureInfo.InvariantCulture)} × {Size.Height.ToString("N0", CultureInfo.InvariantCulture)}";
}

public sealed record FilesContent(IReadOnlyList<string> Paths) : ClipContent
{
    [JsonIgnore]
    public override ClipKind Kind => ClipKind.Files;
    [JsonIgnore]
    public override string DisplayTitle => Paths.Count == 1 ? Format.FileName(Paths[0]) : $"{Paths.Count} Files";
    [JsonIgnore]
    public override string SearchText => string.Join(' ', Paths);
    public override string Footer(long byteCount) => Format.Counted(Paths.Count, "file");

    public bool Equals(FilesContent? other) => other is not null && Paths.SequenceEqual(other.Paths);
    public override int GetHashCode() => Paths.Aggregate(0, HashCode.Combine);
}

public sealed record ColorContent(ColorValue Color) : ClipContent
{
    [JsonIgnore]
    public override ClipKind Kind => ClipKind.Color;
    [JsonIgnore]
    public override string DisplayTitle => Color.Hex;
    [JsonIgnore]
    public override string SearchText => Color.Hex;

    public override string Footer(long byteCount)
    {
        var (r, g, b) = Color.Rgb8;
        return $"RGB {r}, {g}, {b}";
    }
}

public sealed record PdfContent : ClipContent
{
    [JsonIgnore]
    public override ClipKind Kind => ClipKind.Pdf;
    [JsonIgnore]
    public override string DisplayTitle => "PDF Document";
    [JsonIgnore]
    public override string SearchText => Kind.Name();
    public override string Footer(long byteCount) => Format.Bytes(byteCount);
}

public sealed record MediaContent : ClipContent
{
    [JsonIgnore]
    public override ClipKind Kind => ClipKind.Media;
    [JsonIgnore]
    public override string DisplayTitle => "Media";
    [JsonIgnore]
    public override string SearchText => Kind.Name();
    public override string Footer(long byteCount) => Format.Bytes(byteCount);
}

public sealed record DataContent : ClipContent
{
    [JsonIgnore]
    public override ClipKind Kind => ClipKind.Data;
    [JsonIgnore]
    public override string DisplayTitle => "Clipboard Data";
    [JsonIgnore]
    public override string SearchText => Kind.Name();
    public override string Footer(long byteCount) => Format.Bytes(byteCount);
}

public sealed record Clip(
    Guid Id,
    DateTimeOffset CreatedAt,
    SourceApp? Source,
    ClipContent Content,
    string? CustomTitle,
    Guid? PinboardId,
    string ContentHash,
    long ByteCount)
{
    [JsonIgnore]
    public ClipKind Kind => Content.Kind;

    [JsonIgnore]
    public string FooterText => Content.Footer(ByteCount);

    public bool Matches(string query) =>
        new[] { Content.SearchText, CustomTitle, Source?.Name, Kind.Name() }
            .Any(text => text is not null && text.Contains(query, StringComparison.CurrentCultureIgnoreCase));
}

public enum PinboardColor
{
    Red,
    Orange,
    Yellow,
    Green,
    Teal,
    Blue,
    Purple,
    Pink,
}

public sealed record Pinboard(Guid Id, string Name, PinboardColor Color);
