namespace Insert.Core.Tests;

public sealed class RingTests
{
    private static readonly Point2 Origin = new(0, 0);
    private static readonly Rect2 Screen = new(0, 0, 1000, 800);

    private static Point2 At(double degreesClockwiseFromTwelve, double radius = 100)
    {
        var radians = degreesClockwiseFromTwelve * Math.PI / 180 + RingGeometry.FirstDirection;
        return new Point2(Math.Cos(radians) * radius, Math.Sin(radians) * radius);
    }

    [Fact]
    public void SegmentZeroIsAtTwelveAndTheOthersFollowClockwiseWithYDown()
    {
        Assert.Equal(0, RingGeometry.Segment(new Point2(0, -100), Origin, 8));
        Assert.Equal(2, RingGeometry.Segment(new Point2(100, 0), Origin, 8));
        Assert.Equal(4, RingGeometry.Segment(new Point2(0, 100), Origin, 8));
        Assert.Equal(6, RingGeometry.Segment(new Point2(-100, 0), Origin, 8));
        Assert.Equal(1, RingGeometry.Segment(new Point2(100, -100), Origin, 4));
    }

    [Fact]
    public void CenterHoleHasNoSegment()
    {
        Assert.Null(RingGeometry.Segment(new Point2(0, -(RingGeometry.DeadZone - 1)), Origin, 8));
        Assert.Equal(0, RingGeometry.Segment(new Point2(0, -RingGeometry.DeadZone), Origin, 8));
        Assert.Null(RingGeometry.Segment(new Point2(0, -100), Origin, 0));
    }

    [Fact]
    public void AnglesRoundToTheNearestSegment()
    {
        Assert.Equal(0, RingGeometry.Segment(At(22), Origin, 8));
        Assert.Equal(1, RingGeometry.Segment(At(23), Origin, 8));
        Assert.Equal(7, RingGeometry.Segment(At(337), Origin, 8));
        Assert.Equal(0, RingGeometry.Segment(At(338), Origin, 8));
        Assert.Equal(0, RingGeometry.Segment(At(359.9), Origin, 8));
        Assert.Equal(1, RingGeometry.Segment(At(179), Origin, 3));
        Assert.Equal(2, RingGeometry.Segment(At(181), Origin, 3));
    }

    [Fact]
    public void PresentClampsTheCenterIntoTheBounds()
    {
        var margin = RingGeometry.OuterRadius + 8;
        var layout = new RingLayout();

        Assert.Equal(new Point2(margin, margin), layout.Present(8, new Point2(10, -50), Screen));
        Assert.Equal(new Point2(1000 - margin, 800 - margin), layout.Present(8, new Point2(990, 900), Screen));
        Assert.Equal(new Point2(500, 400), layout.Present(8, new Point2(500, 400), Screen));
        Assert.Equal(new Point2(10 + margin, 20 + margin), layout.Present(8, new Point2(50, 50), new Rect2(10, 20, 100, 100)));
        Assert.Null(layout.Selection);
    }

    [Fact]
    public void TrackChangesTheSelectionAndIgnoresPointsOnThePreviewCard()
    {
        var layout = new RingLayout();
        var center = layout.Present(8, new Point2(500, 400), Screen);
        Point2 Offset(Point2 point) => new(center.X + point.X, center.Y + point.Y);

        Assert.True(layout.Track(Offset(At(0))));
        Assert.Equal(0, layout.Selection);
        Assert.False(layout.Track(Offset(At(10))));

        var card = layout.PreviewFrame(0);
        var onCard = new Point2(card.Right - 10, card.Bottom - 10);
        Assert.NotEqual(0, RingGeometry.Segment(onCard, center, 8));
        Assert.False(layout.Track(onCard));
        Assert.Equal(0, layout.Selection);

        Assert.True(layout.Track(Offset(At(90))));
        Assert.Equal(2, layout.Selection);
        Assert.True(layout.Track(center));
        Assert.Null(layout.Selection);
    }

    [Fact]
    public void PreviewGoesToTheOppositeSideWhenTheCardDoesNotFit()
    {
        var layout = new RingLayout();
        var center = layout.Present(8, new Point2(500, 400), Screen);
        var toward = layout.PreviewCenter(0);
        Assert.Equal(center.X, toward.X, 6);
        Assert.True(toward.Y < center.Y);

        center = layout.Present(8, new Point2(500, 0), Screen);
        var flipped = layout.PreviewCenter(0);
        Assert.Equal(center.X, flipped.X, 6);
        Assert.True(flipped.Y > center.Y);
        Assert.True(Screen.Contains(new Point2(flipped.X, flipped.Y - RingGeometry.CardSize / 2)));
    }

    [Fact]
    public void AcceptsClickOnTheRingOrThePreviewWhileASegmentIsSelected()
    {
        var layout = new RingLayout();
        var center = layout.Present(8, new Point2(500, 400), Screen);
        var onRing = new Point2(center.X, center.Y - 100);
        Assert.False(layout.AcceptsClick(onRing));

        layout.Track(onRing);
        var card = layout.PreviewFrame(0);

        Assert.True(layout.AcceptsClick(onRing));
        Assert.True(layout.AcceptsClick(new Point2(card.X + 5, card.Y + 5)));
        Assert.False(layout.AcceptsClick(center));
        Assert.False(layout.AcceptsClick(new Point2(5, 795)));

        layout.Dismiss();
        Assert.False(layout.AcceptsClick(onRing));
    }
}
