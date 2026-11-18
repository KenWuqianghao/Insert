using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Platform.Storage;
using Insert.Core;

namespace Insert.Views;

public sealed partial class SettingsWindow : Window
{
    public SettingsWindow()
    {
        InitializeComponent();
        AddHandler(KeyDownEvent, OnKeyDown, RoutingStrategies.Tunnel);
        Deactivated += (_, _) => StopRecording();
        Closing += (_, e) =>
        {
            StopRecording();
            // A shutdown or a sign-out must close the window. A close by the user only hides it,
            // so that it opens again with the same state.
            if (e.CloseReason != WindowCloseReason.WindowClosing) return;
            e.Cancel = true;
            Hide();
        };
    }

    public SettingsViewModel? ViewModel => DataContext as SettingsViewModel;

    /// <summary>The global shortcut must be off while the user records a new one, or it takes the key press.</summary>
    public Action<bool> RecordingChanged { get; set; } = _ => { };

    public Action ClearHistoryRequested { get; set; } = () => { };

    private void ShowGeneral(object? sender, RoutedEventArgs e) => ViewModel!.IsGeneral = true;

    private void ShowPrivacy(object? sender, RoutedEventArgs e) => ViewModel!.IsGeneral = false;

    private void RecordShortcut(object? sender, RoutedEventArgs e)
    {
        if (ViewModel is not { } model) return;
        if (model.IsRecording)
        {
            StopRecording();
            return;
        }
        model.IsRecording = true;
        RecordingChanged(true);
    }

    private void StopRecording()
    {
        if (ViewModel is not { IsRecording: true } model) return;
        model.IsRecording = false;
        RecordingChanged(false);
    }

    private void OnKeyDown(object? sender, KeyEventArgs e)
    {
        if (ViewModel is not { IsRecording: true } model) return;
        e.Handled = true;
        if (e.Key == Key.Escape)
        {
            StopRecording();
            return;
        }
        if (KeyMap.IsModifier(e.Key)) return;
        var hotkey = KeyMap.VirtualKey(e.Key) is { } key ? new Hotkey(KeyMap.Modifiers(e.KeyModifiers), key) : null;
        if (hotkey is not { IsValid: true }) return;
        model.SetHotkey(hotkey);
        StopRecording();
    }

    private async void AddIgnoredApp(object? sender, RoutedEventArgs e)
    {
        try
        {
            var start = await StorageProvider.TryGetFolderFromPathAsync(Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles));
            var files = await StorageProvider.OpenFilePickerAsync(new FilePickerOpenOptions
            {
                Title = "Ignore Apps",
                AllowMultiple = true,
                SuggestedStartLocation = start,
                FileTypeFilter = [new FilePickerFileType("Apps") { Patterns = ["*.exe"] }],
            });
            foreach (var file in files)
            {
                if (file.TryGetLocalPath() is { } path) ViewModel?.AddIgnoredApp(path);
            }
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException or InvalidOperationException or ArgumentException)
        {
            Services.Log.Info($"settings: could not add an ignored app: {error.Message}");
        }
    }

    private void RemoveIgnoredApp(object? sender, RoutedEventArgs e)
    {
        if ((sender as Control)?.DataContext is IgnoredAppRow row) ViewModel?.RemoveIgnoredApp(row.App);
    }

    private void ClearHistory(object? sender, RoutedEventArgs e) => ClearHistoryRequested();
}
