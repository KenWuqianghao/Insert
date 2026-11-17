using System.Globalization;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using Insert.Core;

namespace Insert.Views;

/// <summary>One segment of the ring: the app icon of the clip and the icon of its kind.</summary>
public sealed record RingItem(Bitmap? AppIcon, Color Tint, Geometry? KindGlyph, string Title, string Time);

/// <summary>Draws the donut: tinted glass, the hairlines, the selected segment, the icons, and the title in the hole.</summary>
public sealed class RingView : Control
{
    private static readonly IBrush Glass = new SolidColorBrush(Color.FromArgb(0xB8, 0x24, 0x24, 0x24));
    private static readonly IPen Hairline = new Pen(new SolidColorBrush(Color.FromArgb(41, 255, 255, 255)), 1);
    private static readonly IBrush SelectionFill = new SolidColorBrush(Color.FromArgb(41, 255, 255, 255));
    private static readonly IBrush BadgeFill = new SolidColorBrush(Color.FromArgb(153, 0, 0, 0));
    private static readonly IPen BadgeStroke = new Pen(new SolidColorBrush(Color.FromArgb(51, 255, 255, 255)), 0.5);
    private static readonly IBrush SecondaryText = new SolidColorBrush(Color.FromArgb(160, 255, 255, 255));

    private IReadOnlyList<RingItem> _items = [];
    private int? _selection;

    public RingView()
    {
        Width = Height = RingGeometry.OuterRadius * 2;
        IsHitTestVisible = false;
    }

    public IReadOnlyList<RingItem> Items
    {
        get => _items;
        set
        {
            _items = value;
            InvalidateVisual();
        }
    }

    public int? Selection
    {
        get => _selection;
        set
        {
            _selection = value;
            InvalidateVisual();
        }
    }

    public override void Render(DrawingContext context)
    {
        var center = new Point(RingGeometry.OuterRadius, RingGeometry.OuterRadius);
        const double outer = RingGeometry.OuterRadius;
        const double inner = RingGeometry.InnerRadius;
        const double hole = RingGeometry.HoleRadius;

        var band = new GeometryGroup { FillRule = FillRule.EvenOdd };
        band.Children.Add(new EllipseGeometry(new Rect(center.X - outer, center.Y - outer, outer * 2, outer * 2)));
        band.Children.Add(new EllipseGeometry(new Rect(center.X - inner, center.Y - inner, inner * 2, inner * 2)));
        context.DrawGeometry(Glass, null, band);
        context.DrawEllipse(Glass, null, center, hole, hole);

        context.DrawEllipse(null, Hairline, center, outer - 0.5, outer - 0.5);
        context.DrawEllipse(null, Hairline, center, inner, inner);
        context.DrawEllipse(null, Hairline, center, hole, hole);

        var count = _items.Count;
        if (_selection is { } selected && selected < count)
        {
            context.DrawGeometry(SelectionFill, null, Sector(center, RingGeometry.Direction(selected, count), RingGeometry.Span(count),
                inner + RingGeometry.SelectionInset, outer - RingGeometry.SelectionInset));
        }

        for (var index = 0; index < count; index++)
        {
            var direction = RingGeometry.Direction(index, count);
            var scale = _selection == index ? RingGeometry.SelectedIconScale : 1;
            var side = RingGeometry.IconSize * scale;
            var iconCenter = new Point(center.X + Math.Cos(direction) * RingGeometry.IconRadius, center.Y + Math.Sin(direction) * RingGeometry.IconRadius);
            DrawIcon(context, _items[index], iconCenter, side, scale);
        }

        if (_selection is { } current && current < count)
        {
            var item = _items[current];
            var width = hole * 2 - 16;
            var title = Text(item.Title, 12, FontWeight.SemiBold, Brushes.White, width);
            var time = Text(item.Time, 10.5, FontWeight.Normal, SecondaryText, width);
            var top = center.Y - (title.Height + 1 + time.Height) / 2;
            context.DrawText(title, new Point(center.X - title.WidthIncludingTrailingWhitespace / 2, top));
            context.DrawText(time, new Point(center.X - time.WidthIncludingTrailingWhitespace / 2, top + title.Height + 1));
        }
    }

    private static void DrawIcon(DrawingContext context, RingItem item, Point center, double side, double scale)
    {
        var rect = new Rect(center.X - side / 2, center.Y - side / 2, side, side);
        if (item.AppIcon is { } icon)
            context.DrawImage(icon, rect);
        else
            context.DrawRectangle(new SolidColorBrush(item.Tint), null, rect, 7 * scale, 7 * scale);

        var badgeSide = 14 * scale;
        var badgeCenter = new Point(rect.Right - badgeSide / 2 + 4 * scale, rect.Bottom - badgeSide / 2 + 4 * scale);
        context.DrawEllipse(BadgeFill, BadgeStroke, badgeCenter, badgeSide / 2, badgeSide / 2);
        if (item.KindGlyph is { } glyph)
        {
            var glyphSide = 8 * scale;
            var bounds = glyph.Bounds;
            var fit = glyphSide / Math.Max(bounds.Width, bounds.Height);
            var matrix = Matrix.CreateTranslation(-bounds.Center.X, -bounds.Center.Y) * Matrix.CreateScale(fit, fit)
                * Matrix.CreateTranslation(badgeCenter.X, badgeCenter.Y);
            using (context.PushTransform(matrix))
                context.DrawGeometry(Brushes.White, null, glyph);
        }
    }

    private static FormattedText Text(string text, double size, FontWeight weight, IBrush brush, double maxWidth)
    {
        var formatted = new FormattedText(text, CultureInfo.CurrentCulture, FlowDirection.LeftToRight,
            new Typeface(FontFamily.Default, FontStyle.Normal, weight), size, brush)
        {
            MaxTextWidth = maxWidth,
            MaxLineCount = 1,
            Trimming = TextTrimming.CharacterEllipsis,
        };
        return formatted;
    }

    /// <summary>An annular sector with a gap of <see cref="RingGeometry.Gap"/> on each side.</summary>
    private static Geometry Sector(Point center, double direction, double span, double inner, double outer)
    {
        if (span >= 2 * Math.PI - 0.001)
        {
            var ring = new GeometryGroup { FillRule = FillRule.EvenOdd };
            ring.Children.Add(new EllipseGeometry(new Rect(center.X - outer, center.Y - outer, outer * 2, outer * 2)));
            ring.Children.Add(new EllipseGeometry(new Rect(center.X - inner, center.Y - inner, inner * 2, inner * 2)));
            return ring;
        }

        double HalfGap(double radius) => RingGeometry.Gap / 2 / radius;
        Point At(double angle, double radius) => new(center.X + Math.Cos(angle) * radius, center.Y + Math.Sin(angle) * radius);

        var outerStart = direction - span / 2 + HalfGap(outer);
        var outerEnd = direction + span / 2 - HalfGap(outer);
        var innerStart = direction + span / 2 - HalfGap(inner);
        var innerEnd = direction - span / 2 + HalfGap(inner);
        var isLarge = span > Math.PI;

        var geometry = new StreamGeometry();
        using var path = geometry.Open();
        path.BeginFigure(At(outerStart, outer), true);
        path.ArcTo(At(outerEnd, outer), new Size(outer, outer), 0, isLarge, SweepDirection.Clockwise);
        path.LineTo(At(innerStart, inner));
        path.ArcTo(At(innerEnd, inner), new Size(inner, inner), 0, isLarge, SweepDirection.CounterClockwise);
        path.EndFigure(true);
        return geometry;
    }
}
