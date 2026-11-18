using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Threading;
using Insert.Core;
using Insert.Platform;
using Insert.Services;

namespace Insert.Views;

/// <summary>The tray at the bottom of the screen. It shows on the monitor of the pointer and takes the keyboard.</summary>
public sealed partial class TrayWindow : Window
{
    private enum Presentation
    {
        Hidden,
        Shown,
        Hiding,
    }

    private readonly IPlatform _platform;
    private readonly PasteService _paste;
    private readonly DispatcherTimer _clock;
    private Presentation _presentation = Presentation.Hidden;
    /// <summary>Counts the hides, so that the timer of an earlier hide does nothing after the tray opened again.</summary>
    private int _hideCount;
    /// <summary>The naming that has the name box. Other model changes keep the text that the user typed.</summary>
    private PanelMode.Naming? _naming;
    private WindowHandle _target = WindowHandle.None;
    private PixelRect _workArea;
    private double _scaling = 1;

    public TrayWindow()
    {
        throw new InvalidOperationException("The designer is not supported.");
    }

    public TrayWindow(TrayViewModel viewModel, IPlatform platform, PasteService paste)
    {
        InitializeComponent();
        ViewModel = viewModel;
        DataContext = viewModel;
        _platform = platform;
        _paste = paste;

        var model = viewModel.Model;
        model.Hide = () => HideTray();
        model.ActivateClip = Activate;
        viewModel.PrimaryMoved += index => Dispatcher.UIThread.Post(() => Strip.ScrollIntoView(index), DispatcherPriority.Background);
        viewModel.PropertyChanged += (_, args) =>
        {
            if (args.PropertyName == nameof(TrayViewModel.IsPreviewing)) Place();
            if (args.PropertyName == nameof(TrayViewModel.IsNaming)) FocusForMode();
        };

        AddHandler(KeyDownEvent, OnKeyDown, RoutingStrategies.Tunnel);
        AddHandler(KeyUpEvent, OnKeyUp, RoutingStrategies.Tunnel);
        Deactivated += (_, _) => HideTray();
        ScalingChanged += (_, _) => Place();
        PropertyChanged += (_, args) =>
        {
            if (args.Property == ActualTransparencyLevelProperty) UpdateGlass();
        };

        _clock = new DispatcherTimer(TimeSpan.FromSeconds(30), DispatcherPriority.Background, (_, _) => viewModel.RefreshTimes());
    }

    public TrayViewModel ViewModel { get; }

    public Action ShowSettings { get; set; } = () => { };
    public Action ClearHistory { get; set; } = () => { };
    public Action Quit { get; set; } = () => { };
    public Action<bool> SetCapturePaused { get; set; } = _ => { };

    public bool IsShown => _presentation == Presentation.Shown;

    public void Toggle()
    {
        if (_presentation == Presentation.Shown)
            HideTray();
        else
            ShowTray();
    }

    public void ShowTray()
    {
        if (_presentation == Presentation.Shown) return;
        _target = _platform.Apps.ForegroundWindow();
        _presentation = Presentation.Shown;
        _naming = null;

        var pointer = _platform.Windows.Pointer;
        var screen = Screens.ScreenFromPoint(pointer) ?? Screens.Primary;
        _workArea = screen?.WorkingArea ?? new PixelRect(0, 0, 1280, 800);
        _scaling = screen?.Scaling ?? 1;

        ViewModel.Model.PrepareToShow();
        ViewModel.RefreshTimes();
        Body.Classes.Add("hidden");
        Show();
        Place();
        UpdateGlass();
        _platform.Windows.Activate(this);
        Dispatcher.UIThread.Post(() => Body.Classes.Remove("hidden"), DispatcherPriority.Render);
        FocusForMode();
        _clock.Start();
        Log.Info($"tray: shown on a {_workArea.Width}x{_workArea.Height} work area at scale {_scaling}");
    }

    public void HideTray(Action? then = null)
    {
        if (_presentation != Presentation.Shown) return;
        _presentation = Presentation.Hiding;
        var hide = ++_hideCount;
        _clock.Stop();
        Body.Classes.Add("hidden");
        DispatcherTimer.RunOnce(() =>
        {
            if (_presentation != Presentation.Hiding || hide != _hideCount) return;
            Hide();
            _presentation = Presentation.Hidden;
            ViewModel.Model.DidHide();
            then?.Invoke();
        }, TimeSpan.FromMilliseconds(160));
    }

    private void Activate(Clip clip, PasteAction action)
    {
        var target = _target;
        HideTray(() =>
        {
            if (!_paste.Perform(action, clip, target)) Log.Info("paste: the payload file is missing");
        });
    }

    /// <summary>The tray fills the width of the work area. The preview adds height above the cards.</summary>
    private void Place()
    {
        if (_presentation != Presentation.Shown) return;
        var height = Metrics.BaseHeight;
        if (ViewModel.IsPreviewing)
            height += Math.Min(_workArea.Height / _scaling * Metrics.PreviewScreenFraction, Metrics.PreviewMaxHeight);
        var pixels = (int)Math.Round(height * _scaling);
        _platform.Windows.Place(this, new PixelRect(_workArea.X, _workArea.Bottom - pixels, _workArea.Width, pixels));
    }

    private void UpdateGlass()
    {
        Glass.Background = ActualTransparencyLevel == WindowTransparencyLevel.None
            ? (IBrushResource("TraySolid"))
            : (IBrushResource("TrayGlassTint"));
    }

    private Avalonia.Media.IBrush? IBrushResource(string key) =>
        this.TryFindResource(key, ActualThemeVariant, out var value) ? value as Avalonia.Media.IBrush : null;

    private void FocusForMode()
    {
        Dispatcher.UIThread.Post(() =>
        {
            var model = ViewModel.Model;
            if (model.Mode is PanelMode.Naming naming)
            {
                if (naming == _naming) return;
                _naming = naming;
                NameBox.Text = model.CurrentName(naming.Target);
                NameBox.Focus();
                NameBox.SelectAll();
            }
            else
            {
                _naming = null;
                SearchBox.Focus();
            }
        }, DispatcherPriority.Input);
    }

    private void OnKeyDown(object? sender, KeyEventArgs e)
    {
        UpdateControlHeld(e.KeyModifiers, e.Key, isDown: true);
        if (KeyMap.IsModifier(e.Key)) return;

        var model = ViewModel.Model;
        var command = PanelCommand.From(KeyMap.Press(e.Key, e.KeyModifiers), model.Mode, model.Query.Length == 0);
        if (command is null) return;
        e.Handled = true;
        model.Perform(command);
    }

    private void OnKeyUp(object? sender, KeyEventArgs e) => UpdateControlHeld(e.KeyModifiers, e.Key, isDown: false);

    private void UpdateControlHeld(KeyModifiers modifiers, Key key, bool isDown)
    {
        var held = key is Key.LeftCtrl or Key.RightCtrl ? isDown : modifiers.HasFlag(KeyModifiers.Control);
        ViewModel.Model.IsControlHeld = held && IsActive;
    }

    private void NameKeyDown(object? sender, KeyEventArgs e)
    {
        if (e.Key != Key.Enter) return;
        e.Handled = true;
        ViewModel.Model.CommitName(NameBox.Text ?? "");
    }

    private void ClearQuery(object? sender, RoutedEventArgs e)
    {
        ViewModel.Query = "";
        SearchBox.Focus();
    }

    private void ResumeCapture(object? sender, RoutedEventArgs e) => SetCapturePaused(false);

    private void SelectScope(object? sender, RoutedEventArgs e)
    {
        if ((sender as Control)?.DataContext is ScopeTabViewModel tab) ViewModel.Model.Scope = tab.Scope;
        SearchBox.Focus();
    }

    private void NewPinboard(object? sender, RoutedEventArgs e) =>
        ViewModel.Model.BeginNaming(new NameTarget.NewPinboard(new HashSet<Guid>()));

    private void CardPressed(object? sender, PointerPressedEventArgs e)
    {
        if ((sender as Control)?.DataContext is not CardViewModel card) return;
        var point = e.GetCurrentPoint(this);
        var model = ViewModel.Model;
        var shift = e.KeyModifiers.HasFlag(KeyModifiers.Shift);
        var control = e.KeyModifiers.HasFlag(KeyModifiers.Control);

        if (point.Properties.IsRightButtonPressed)
        {
            if (!model.Selection.Contains(card.Clip.Id)) model.Click(card.Clip.Id, shift: false, control: false);
            return;
        }
        if (!point.Properties.IsLeftButtonPressed) return;

        if (e.ClickCount == 2 && !shift && !control)
            model.ActivateClip(card.Clip, PasteAction.Paste);
        else
            model.Click(card.Clip.Id, shift, control);
        e.Handled = true;
    }

    private void CardMenu(object? sender, ContextRequestedEventArgs e)
    {
        if (sender is not Control control || control.DataContext is not CardViewModel card) return;
        e.Handled = true;
        Menus.ForClip(ViewModel.Model, card.Clip).Open(control);
    }

    private void ScopeMenu(object? sender, ContextRequestedEventArgs e)
    {
        if (sender is not Control control || control.DataContext is not ScopeTabViewModel { Pinboard: { } pinboard }) return;
        e.Handled = true;
        Menus.ForPinboard(ViewModel.Model, pinboard).Open(control);
    }

    private void ShowFilterMenu(object? sender, RoutedEventArgs e) => Menus.ForKindFilter(ViewModel.Model).Open(FilterButton);

    private void ShowMoreMenu(object? sender, RoutedEventArgs e) =>
        Menus.More(ViewModel.CapturePaused, ShowSettings, () => SetCapturePaused(!ViewModel.CapturePaused), ClearHistory, Quit).Open(MoreButton);
}
