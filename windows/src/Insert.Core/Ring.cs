namespace Insert.Core;

public readonly record struct Rect2(double X, double Y, double Width, double Height)
{
    public double Right => X + Width;
    public double Bottom => Y + Height;

    public bool Contains(Point2 point) => point.X >= X && point.X <= Right && point.Y >= Y && point.Y <= Bottom;
}

/// <summary>The sizes of the clip ring in device-independent pixels. The y axis points down.</summary>
public static class RingGeometry
{
    public const int Capacity = 8;
    public const double InnerRadius = 72;
    public const double OuterRadius = 132;
    public const double IconRadius = 102;
    public const double HoleRadius = 62;
    public const double DeadZone = HoleRadius;
    public const double IconSize = 30;
    public const double SelectedIconScale = 1.28;
    public const double SelectionInset = 3;
    public const double Gap = 6;
    public const double PreviewGap = 22;
    public const double CardSize = 232;

    /// <summary>Segment 0 is at 12 o'clock. The next segments follow clockwise.</summary>
    public const double FirstDirection = -Math.PI / 2;

    public static double Direction(int index, int count) => FirstDirection + index * Span(count);

    public static double Span(int count) => 2 * Math.PI / Math.Max(count, 1);

    /// <summary>The segment under the point, or null in the center hole.</summary>
    public static int? Segment(Point2 point, Point2 center, int count)
    {
        var dx = point.X - center.X;
        var dy = point.Y - center.Y;
        if (count <= 0 || Math.Sqrt(dx * dx + dy * dy) < DeadZone) return null;

        var angle = Math.Atan2(dy, dx) - FirstDirection;
        if (angle < 0) angle += 2 * Math.PI;
        return (int)Math.Round(angle / Span(count), MidpointRounding.AwayFromZero) % count;
    }
}

/// <summary>Where the ring and its preview card go inside the work area of a monitor.</summary>
public sealed class RingLayout
{
    public int Count { get; private set; }
    public Point2 Center { get; private set; }
    public Rect2 Bounds { get; private set; }
    public int? Selection { get; private set; }

    /// <summary>Puts the ring at the pointer, moved in so that the full ring is inside <paramref name="bounds"/>. Returns the center.</summary>
    public Point2 Present(int count, Point2 pointer, Rect2 bounds)
    {
        var margin = RingGeometry.OuterRadius + 8;
        Count = count;
        Bounds = bounds;
        Center = new Point2(
            Math.Clamp(pointer.X, bounds.X + margin, Math.Max(bounds.X + margin, bounds.Right - margin)),
            Math.Clamp(pointer.Y, bounds.Y + margin, Math.Max(bounds.Y + margin, bounds.Bottom - margin)));
        Selection = null;
        return Center;
    }

    /// <summary>Selects the segment under the pointer. Returns true when the selection changed.</summary>
    public bool Track(Point2 point)
    {
        if (IsOnPreviewCard(point)) return false;
        var index = RingGeometry.Segment(point, Center, Count);
        if (index == Selection) return false;
        Selection = index;
        return true;
    }

    public void Dismiss() => Selection = null;

    public bool AcceptsClick(Point2 point) =>
        Selection is { } selection && (SegmentAreaContains(point) || PreviewFrame(selection).Contains(point));

    public Point2 PreviewCenter(int index)
    {
        var angle = RingGeometry.Direction(index, Count);
        var toward = PreviewPlacement(angle);
        var away = PreviewPlacement(angle + Math.PI);
        return toward.Fits || !away.Fits ? toward.Center : away.Center;
    }

    public Rect2 PreviewFrame(int index)
    {
        const double side = RingGeometry.CardSize;
        var center = PreviewCenter(index);
        return new Rect2(center.X - side / 2, center.Y - side / 2, side, side);
    }

    private bool IsOnPreviewCard(Point2 point) =>
        Selection is { } selection && PreviewFrame(selection).Contains(point) && !SegmentAreaContains(point);

    private bool SegmentAreaContains(Point2 point)
    {
        var distance = point.DistanceTo(Center);
        return distance >= RingGeometry.InnerRadius && distance <= RingGeometry.OuterRadius;
    }

    private (Point2 Center, bool Fits) PreviewPlacement(double angle)
    {
        const double half = RingGeometry.CardSize / 2;
        var cardExtent = half * (Math.Abs(Math.Cos(angle)) + Math.Abs(Math.Sin(angle)));
        var reach = RingGeometry.OuterRadius + RingGeometry.PreviewGap + cardExtent;
        var wanted = new Point2(Center.X + Math.Cos(angle) * reach, Center.Y + Math.Sin(angle) * reach);
        const double inset = half + 12;
        var placed = new Point2(
            Math.Clamp(wanted.X, Bounds.X + inset, Math.Max(Bounds.X + inset, Bounds.Right - inset)),
            Math.Clamp(wanted.Y, Bounds.Y + inset, Math.Max(Bounds.Y + inset, Bounds.Bottom - inset)));
        return (placed, Math.Abs(placed.X - wanted.X) < 1e-9 && Math.Abs(placed.Y - wanted.Y) < 1e-9);
    }
}
