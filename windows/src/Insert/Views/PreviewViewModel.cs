using Avalonia.Media;
using Avalonia.Media.Imaging;
using Insert.Core;
using Insert.Services;

namespace Insert.Views;

public abstract record PreviewBody;

public sealed record TextPreview(string Text) : PreviewBody;

public sealed record ImagePreview(Bitmap? Image) : PreviewBody;

public sealed record LinkPreview(string? Title, string Url) : PreviewBody;

public sealed record FileRow(Bitmap? Icon, Geometry? Glyph, string Name, string Folder);

public sealed record FilesPreview(IReadOnlyList<FileRow> Files) : PreviewBody;

public sealed record ColorPreview(IBrush Fill, string Hex, string Detail, IBrush Foreground) : PreviewBody;

public sealed record PlaceholderPreview(Geometry? Glyph, string Label) : PreviewBody;

/// <summary>The large preview above the cards. It reads the payload file for long text and for the full image.</summary>
public sealed class PreviewViewModel
{
    private const int TextLimit = 200_000;

    public PreviewViewModel(Clip clip, ClipLibrary library, IconStore icons, Bitmap? thumbnail)
    {
        Clip = clip;
        AppIcon = icons.For(clip.Source).Icon;
        Title = clip.CustomTitle ?? clip.Content.DisplayTitle;
        Detail = string.Join(" · ", new[] { clip.Kind.Name(), clip.Source?.Name, clip.FooterText }.Where(part => part is not null));
        Body = clip.Content switch
        {
            TextContent text => new TextPreview(FullText(text.Snippet, clip, library)),
            RichTextContent text => new TextPreview(FullText(text.Snippet, clip, library)),
            ImageContent => new ImagePreview(FullImage(clip, library) ?? thumbnail),
            LinkContent link => new LinkPreview(link.Title, link.Url),
            FilesContent files => new FilesPreview(files.Paths.Select(path =>
                new FileRow(icons.FileIcon(path), Palette.Icon("Document"), Format.FileName(path), Format.DirectoryName(path) ?? "")).ToList()),
            ColorContent color => new ColorPreview(
                new SolidColorBrush(Palette.ToColor(color.Color)), color.Color.Hex, clip.FooterText,
                color.Color.IsLight ? new SolidColorBrush(Color.FromArgb(191, 0, 0, 0)) : Brushes.White),
            _ => new PlaceholderPreview(Palette.KindIcon(clip.Kind), $"{clip.Content.DisplayTitle}, {clip.FooterText}"),
        };
    }

    public Clip Clip { get; }
    public Bitmap? AppIcon { get; }
    public string Title { get; }
    public string Detail { get; }
    public PreviewBody Body { get; }

    private static string FullText(TextSnippet snippet, Clip clip, ClipLibrary library)
    {
        if (snippet.CharacterCount <= TextSnippet.Limit || library.Payload(clip.Id)?.PlainText is not { } text) return snippet.Text;
        return text.Length > TextLimit ? text[..TextLimit] : text;
    }

    private static Bitmap? FullImage(Clip clip, ClipLibrary library)
    {
        if (library.Payload(clip.Id)?.ImageData is not { } data) return null;
        try
        {
            return new Bitmap(new MemoryStream(data));
        }
        catch (Exception error) when (error is ArgumentException or InvalidOperationException or NotSupportedException or IOException)
        {
            return null;
        }
    }
}
