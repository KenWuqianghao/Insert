using Avalonia.Media;
using Avalonia.Media.Imaging;
using CommunityToolkit.Mvvm.ComponentModel;
using Insert.Core;
using Insert.Services;

namespace Insert.Views;

/// <summary>The body of a card. Each kind has a data template in <c>ClipCardView.axaml</c>.</summary>
public abstract record CardBody;

public sealed record TextBody(string Text) : CardBody;

public sealed record LinkBody(string Title, string Url) : CardBody;

public sealed record ImageBody(Bitmap Thumbnail) : CardBody;

public sealed record ColorBody(IBrush Fill, string Hex, IBrush Foreground) : CardBody;

public sealed record FilesBody(Bitmap? Icon, Geometry? Glyph, IReadOnlyList<string> Names, string? More) : CardBody;

public sealed record PlaceholderBody(Geometry? Glyph, string Label) : CardBody;

public sealed partial class CardViewModel : ObservableObject
{
    private readonly IconStore _icons;

    public CardViewModel(Clip clip, IconStore icons, Func<Guid, Bitmap?> thumbnail)
    {
        _icons = icons;
        Clip = clip;
        var app = icons.For(clip.Source);
        AppIcon = app.Icon;
        HeaderBrush = new SolidColorBrush(app.Tint);
        KindGlyph = Palette.KindIcon(clip.Kind);
        Body = MakeBody(clip, thumbnail);
        Title = clip.CustomTitle ?? clip.Kind.Name();
        Footer = clip.FooterText;
        RefreshTime(DateTimeOffset.Now);
    }

    public Clip Clip { get; private set; }
    public Bitmap? AppIcon { get; }
    public bool HasAppIcon => AppIcon is not null;
    public IBrush HeaderBrush { get; }
    public Geometry? KindGlyph { get; }
    public CardBody Body { get; }

    [ObservableProperty]
    public partial string Title { get; set; }

    [ObservableProperty]
    public partial string Subtitle { get; set; } = "";

    [ObservableProperty]
    public partial string Footer { get; set; }

    [ObservableProperty]
    public partial IBrush? PinBrush { get; set; }

    [ObservableProperty]
    public partial bool IsSelected { get; set; }

    [ObservableProperty]
    public partial bool IsPrimary { get; set; }

    [ObservableProperty]
    public partial bool IsDeleting { get; set; }

    [ObservableProperty]
    public partial string? Badge { get; set; }

    public Avalonia.Thickness SelectionThickness => new(IsPrimary ? 3.5 : 2.5);
    public double SelectionOpacity => IsPrimary ? 1 : 0.55;

    partial void OnIsPrimaryChanged(bool value)
    {
        OnPropertyChanged(nameof(SelectionThickness));
        OnPropertyChanged(nameof(SelectionOpacity));
    }

    /// <summary>A clip with the same id can change its title, time, or pinboard.</summary>
    public void Update(Clip clip, Pinboard? pinboard)
    {
        Clip = clip;
        Title = clip.CustomTitle ?? clip.Kind.Name();
        Footer = clip.FooterText;
        PinBrush = pinboard is null ? null : Palette.PinboardBrush(pinboard.Color);
        RefreshTime(DateTimeOffset.Now);
    }

    public void RefreshTime(DateTimeOffset now)
    {
        var time = Format.RelativeTime(Clip.CreatedAt, now);
        Subtitle = Clip.CustomTitle is null ? time : $"{Clip.Kind.Name()} · {time}";
    }

    private CardBody MakeBody(Clip clip, Func<Guid, Bitmap?> thumbnail) => clip.Content switch
    {
        TextContent text => new TextBody(Excerpt(text.Snippet.Text)),
        RichTextContent text => new TextBody(Excerpt(text.Snippet.Text)),
        LinkContent link => new LinkBody(link.Title ?? link.Host ?? link.Url, link.Url),
        ImageContent when thumbnail(clip.Id) is { } image => new ImageBody(image),
        ImageContent => new PlaceholderBody(Palette.KindIcon(ClipKind.Image), "Image"),
        ColorContent color => new ColorBody(
            new SolidColorBrush(Palette.ToColor(color.Color)),
            color.Color.Hex,
            color.Color.IsLight ? new SolidColorBrush(Color.FromArgb(191, 0, 0, 0)) : Brushes.White),
        FilesContent files => new FilesBody(
            files.Paths.Count > 0 ? _icons.FileIcon(files.Paths[0]) : null,
            Palette.Icon("Document"),
            files.Paths.Take(3).Select(Format.FileName).ToList(),
            files.Paths.Count > 3 ? $"and {files.Paths.Count - 3} more" : null),
        _ => new PlaceholderBody(Palette.KindIcon(clip.Kind), clip.Content.DisplayTitle),
    };

    private static string Excerpt(string text)
    {
        var start = text.Length > 600 ? text[..600] : text;
        return start.Trim();
    }
}
