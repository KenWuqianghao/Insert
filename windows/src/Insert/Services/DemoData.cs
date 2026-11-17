using System.Text;
using Avalonia;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using Insert.Core;

namespace Insert.Services;

/// <summary>Sample clips for screenshots and tests. Insert adds them only with <c>--demo</c>, and only to an empty library.</summary>
public static class DemoData
{
    public static void Seed(ClipLibrary library, IImageCodec images, Func<string, SourceApp> app)
    {
        if (library.Clips.Count > 0) return;
        var now = DateTimeOffset.Now;
        var snippets = library.AddPinboard("Snippets");
        var design = library.AddPinboard("Design");
        library.SetPinboardColor(snippets, PinboardColor.Blue);
        library.SetPinboardColor(design, PinboardColor.Red);

        Add(library, Text("Thanks for the review! I will merge it after the build passes."), app("kate"), now.AddHours(-26), snippets);
        Add(library, Files(@"C:\Users\Public\Documents\Launch plan.pdf", @"C:\Users\Public\Pictures\Screenshots.zip"), app("ark"), now.AddHours(-2), null);
        Add(library, Text("Lunch at 12:30? The usual place."), app("kwrite"), now.AddMinutes(-48), null);
        Add(library, Text("#5B8DEF"), app("filelight"), now.AddMinutes(-26), design);
        Add(library, Image(), app("gwenview"), now.AddMinutes(-11), design, images);
        Add(library, Link("https://github.com/KenWuqianghao/Insert", "Insert: a native clipboard tray for Windows"), app("steam"), now.AddMinutes(-4), null);
        Add(library, Text("What is new in Insert:\r\n\r\n• A tray of cards at the bottom of the screen\r\n• Pinboards for the clips that you use frequently\r\n• A large preview when you press Space\r\n• A clip ring at the pointer when you hold Alt"),
            app("kate"), now.AddSeconds(-20), null);
    }

    private static void Add(ClipLibrary library, ClipPayload payload, SourceApp source, DateTimeOffset time, Guid? pinboard, IImageCodec? images = null)
    {
        if (ClipCapture.Build(payload, source, images ?? NoImages.Instance, time) is not { } capture) return;
        library.Insert(capture);
        if (pinboard is { } id) library.SetPinboard(id, new HashSet<Guid> { capture.Clip.Id });
    }

    private static ClipPayload Text(string text) => ClipPayload.FromText(text);

    private static ClipPayload Link(string url, string title)
    {
        var html = $"Version:0.9\r\nStartHTML:0\r\nEndHTML:0\r\nStartFragment:0\r\nEndFragment:0\r\n<html><body><!--StartFragment--><a href=\"{url}\">{title}</a><!--EndFragment--></body></html>";
        return new ClipPayload([
            new Representation(ClipFormats.UnicodeText, Encoding.Unicode.GetBytes(url)),
            new Representation(ClipFormats.Html, Encoding.UTF8.GetBytes(html)),
        ]);
    }

    private static ClipPayload Files(params string[] paths) =>
        new([new Representation(ClipFormats.FileDrop, Encoding.UTF8.GetBytes(string.Join('\n', paths)))]);

    /// <summary>A sunset over mountains, drawn at 1,600 × 1,000 pixels.</summary>
    private static ClipPayload Image()
    {
        var size = new Avalonia.PixelSize(1600, 1000);
        using var bitmap = new RenderTargetBitmap(size);
        using (var context = bitmap.CreateDrawingContext())
        {
            var sky = new LinearGradientBrush
            {
                StartPoint = new RelativePoint(0, 0, RelativeUnit.Relative),
                EndPoint = new RelativePoint(0, 1, RelativeUnit.Relative),
                GradientStops =
                {
                    new GradientStop(Color.Parse("#F6B07A"), 0),
                    new GradientStop(Color.Parse("#E8775F"), 0.55),
                    new GradientStop(Color.Parse("#9B4F84"), 1),
                },
            };
            context.DrawRectangle(sky, null, new Rect(0, 0, 1600, 1000));
            context.DrawEllipse(new SolidColorBrush(Color.Parse("#FBE3B2")), null, new Point(1030, 430), 120, 120);
            context.DrawGeometry(new SolidColorBrush(Color.Parse("#5A3F86")), null,
                StreamGeometry.Parse("M0,760 L330,470 L560,700 L820,420 L1120,720 L1340,560 L1600,760 L1600,1000 L0,1000 Z"));
            context.DrawGeometry(new SolidColorBrush(Color.Parse("#2F2659")), null,
                StreamGeometry.Parse("M0,880 L260,700 L520,860 L860,640 L1180,880 L1420,760 L1600,860 L1600,1000 L0,1000 Z"));
        }
        using var stream = new MemoryStream();
        bitmap.Save(stream, PngBitmapEncoderOptions.Default);
        return new ClipPayload([new Representation(ClipFormats.Png, stream.ToArray())]);
    }

    private sealed class NoImages : IImageCodec
    {
        public static readonly NoImages Instance = new();

        public (Core.PixelSize Size, byte[] ThumbnailPng)? Inspect(byte[] encoded, int maxThumbnailSide) => null;
    }
}
