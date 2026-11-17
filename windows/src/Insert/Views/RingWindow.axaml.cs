using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Threading;
using Insert.Core;
using Insert.Platform;
using Insert.Services;

namespace Insert.Views;

/// <summary>
/// The clip ring around the pointer. The window covers the work area of one monitor and never takes the keyboard,
/// so the app in front keeps it. On Windows the keyboard hook gives key presses to <see cref="HandleKey"/>.
/// </summary>
public sealed partial class RingWindow : Window
{
    private enum Presentation
    {
        Hidden,
        Shown,
        Hiding,
    }

    private readonly RingLayout _layout = new();
    private readonly ClipLibrary _library;
    private readonly IconStore _icons;
    private readonly IPlatform _platform;
    private readonly PasteService _paste;
    private readonly Func<Guid, Avalonia.Media.Imaging.Bitmap?> _thumbnail;
    private IReadOnlyList<Clip> _clips = [];
    private Presentation _presentation = Presentation.Hidden;
    private double _scaling = 1;

    public RingWindow()
    {
        throw new InvalidOperationException("The designer is not supported.");
    }

    public RingWindow(ClipLibrary library, IconStore icons, IPlatform platform, PasteService paste, Func<Guid, Avalonia.Media.Imaging.Bitmap?> thumbnail)
    {
        InitializeComponent();
        _library = library;
        _icons = icons;
        _platform = platform;
        _paste = paste;
        _thumbnail = thumbnail;

        PointerMoved += (_, e) => Track(e.GetPosition(this));
        PointerPressed += OnPointerPressed;
        KeyDown += (_, e) =>
        {
            e.Handled = true;
            HandleKey(KeyMap.Ring(e.Key));
        };
        Opened += (_, _) => _platform.Windows.MakeNonActivating(this);
    }

    public bool IsShown => _presentation == Presentation.Shown;

    /// <summary>Opens the ring at the pointer. Returns false when there is no clip.</summary>
    public bool ShowRing()
    {
        if (_presentation != Presentation.Hidden) return false;
        _clips = _library.Clips.Take(RingGeometry.Capacity).ToList();
        if (_clips.Count == 0) return false;

        var pointer = _platform.Windows.Pointer;
        var screen = Screens.ScreenFromPoint(pointer) ?? Screens.Primary;
        if (screen is null) return false;
        var work = screen.WorkingArea;
        _scaling = screen.Scaling;

        var bounds = new Rect2(0, 0, work.Width / _scaling, work.Height / _scaling);
        var local = new Point2((pointer.X - work.X) / _scaling, (pointer.Y - work.Y) / _scaling);
        var center = _layout.Present(_clips.Count, local, bounds);
        if (center.DistanceTo(local) > 0.5)
            _platform.Windows.MovePointer(new PixelPoint(work.X + (int)Math.Round(center.X * _scaling), work.Y + (int)Math.Round(center.Y * _scaling)));

        var now = DateTimeOffset.Now;
        Ring.Items = _clips.Select(clip =>
        {
            var app = _icons.For(clip.Source);
            return new RingItem(app.Icon, app.Tint, Palette.KindIcon(clip.Kind), clip.CustomTitle ?? clip.Kind.Name(), Format.RelativeTime(clip.CreatedAt, now));
        }).ToList();
        Ring.Selection = null;
        Canvas.SetLeft(Ring, center.X - RingGeometry.OuterRadius);
        Canvas.SetTop(Ring, center.Y - RingGeometry.OuterRadius);
        PreviewCard.Opacity = 0;
        PreviewCard.DataContext = null;

        _presentation = Presentation.Shown;
        Show();
        _platform.Windows.Place(this, work);
        _platform.Keyboard.KeyFilter = FilterKey;
        // Windows opens the menu bar after a lone Alt release, and Start after a lone Windows key release. A key between stops both.
        _platform.Keyboard.SuppressModifierRelease();
        Dispatcher.UIThread.Post(() => Ring.Classes.Remove("hidden"), DispatcherPriority.Render);
        Log.Info($"ring: shown with {_clips.Count} clips at ({center.X:0}, {center.Y:0}) on a {work.Width}x{work.Height} work area");
        return true;
    }

    /// <summary>The user released the modifier. With a selected segment, the ring pastes it. Without one, the ring stays open for a click.</summary>
    public void EndHold()
    {
        if (IsShown && _layout.Selection is { } index) Paste(index);
    }

    /// <summary>Selects a segment, for tests and screenshots.</summary>
    public void Select(int index)
    {
        if (!IsShown || index < 0 || index >= _clips.Count) return;
        var angle = RingGeometry.Direction(index, _clips.Count);
        var reach = (RingGeometry.InnerRadius + RingGeometry.OuterRadius) / 2;
        Track(new Point(_layout.Center.X + Math.Cos(angle) * reach, _layout.Center.Y + Math.Sin(angle) * reach));
    }

    public void HideRing(Action? then = null)
    {
        if (_presentation != Presentation.Shown) return;
        _presentation = Presentation.Hiding;
        _platform.Keyboard.KeyFilter = null;
        Ring.Classes.Add("hidden");
        PreviewCard.Opacity = 0;
        DispatcherTimer.RunOnce(() =>
        {
            Hide();
            _layout.Dismiss();
            _presentation = Presentation.Hidden;
            then?.Invoke();
        }, TimeSpan.FromMilliseconds(120));
    }

    /// <summary>Called inside the keyboard hook: it must return fast, so the work runs later.</summary>
    private bool FilterKey(RingKey key)
    {
        if (!IsShown) return false;
        Dispatcher.UIThread.Post(() => HandleKey(key));
        return true;
    }

    public void HandleKey(RingKey key)
    {
        if (!IsShown) return;
        switch (key.Kind)
        {
            case RingKeyKind.Escape:
                HideRing();
                break;
            case RingKeyKind.Enter:
                if (_layout.Selection is { } index) Paste(index);
                break;
            case RingKeyKind.Digit when key.Digit >= 1 && key.Digit <= RingGeometry.Capacity:
                Paste(key.Digit - 1);
                break;
            default:
                HideRing();
                break;
        }
    }

    private void Track(Point point)
    {
        if (!IsShown || !_layout.Track(new Point2(point.X, point.Y))) return;
        Ring.Selection = _layout.Selection;
        if (_layout.Selection is { } index)
        {
            var clip = _clips[index];
            var card = new CardViewModel(clip, _icons, _thumbnail);
            card.Update(clip, _library.Pinboards.FirstOrDefault(pinboard => pinboard.Id == clip.PinboardId));
            PreviewCard.DataContext = card;
            var center = _layout.PreviewCenter(index);
            Canvas.SetLeft(PreviewCard, center.X - RingGeometry.CardSize / 2);
            Canvas.SetTop(PreviewCard, center.Y - RingGeometry.CardSize / 2);
            PreviewCard.Opacity = 1;
        }
        else
        {
            PreviewCard.Opacity = 0;
        }
    }

    private void OnPointerPressed(object? sender, PointerPressedEventArgs e)
    {
        if (!IsShown) return;
        var point = e.GetCurrentPoint(this);
        if (point.Properties.IsRightButtonPressed)
        {
            HideRing();
            return;
        }
        var position = new Point2(point.Position.X, point.Position.Y);
        if (_layout.Selection is { } index && _layout.AcceptsClick(position))
            Paste(index);
        else
            HideRing();
    }

    private void Paste(int index)
    {
        if (index < 0 || index >= _clips.Count) return;
        var clip = _clips[index];
        Log.Info($"ring: paste clip {index + 1}");
        HideRing(() =>
        {
            if (!_paste.Perform(PasteAction.Paste, clip, WindowHandle.None)) Log.Info("paste: the payload file is missing");
        });
    }
}
