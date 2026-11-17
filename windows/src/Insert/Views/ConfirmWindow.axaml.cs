using Avalonia.Controls;
using Avalonia.Interactivity;

namespace Insert.Views;

/// <summary>Asks before Insert clears the history. Insert has no main window to own a modal dialog, so this window shows alone.</summary>
public sealed partial class ConfirmWindow : Window
{
    private readonly TaskCompletionSource<bool> _answer = new();
    private bool _confirmed;

    public ConfirmWindow()
    {
        InitializeComponent();
        Closed += (_, _) => _answer.TrySetResult(_confirmed);
    }

    public Task<bool> Ask()
    {
        Show();
        Activate();
        return _answer.Task;
    }

    private void Confirm(object? sender, RoutedEventArgs e)
    {
        _confirmed = true;
        Close();
    }

    private void Cancel(object? sender, RoutedEventArgs e) => Close();
}
