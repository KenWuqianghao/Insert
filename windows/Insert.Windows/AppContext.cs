using System;
using System.Drawing;
using System.Windows.Forms;

namespace Insert.Windows;

sealed class AppContext : ApplicationContext
{
    private readonly NotifyIcon _notifyIcon;
    private readonly OverlayForm _overlay;
    private readonly ClipboardHistory _history;
    private readonly HotkeySettingsStore _hotkeyStore;
    private readonly StartupManager _startupManager;
    private GlobalHotkey _hotkey;
    private HotkeySettings _hotkeySettings;

    public AppContext()
    {
        _hotkeyStore = new HotkeySettingsStore();
        _startupManager = new StartupManager();
        _hotkeySettings = _hotkeyStore.Load();
        _history = new ClipboardHistory();
        _overlay = new OverlayForm(_history);
        _hotkey = RegisterHotkey(_hotkeySettings.Binding);

        _notifyIcon = new NotifyIcon
        {
            Visible = true,
            Text = "Insert",
            Icon = SystemIcons.Application,
            ContextMenuStrip = BuildMenu()
        };

        _notifyIcon.DoubleClick += (_, _) => _overlay.Toggle();

        _history.Changed += (_, _) =>
        {
            if (_overlay.Visible)
            {
                _overlay.RefreshHistory();
            }
        };

        Application.ApplicationExit += (_, _) =>
        {
            _hotkey.Dispose();
            _history.Dispose();
            _notifyIcon.Visible = false;
            _notifyIcon.Dispose();
        };
    }

    private ContextMenuStrip BuildMenu()
    {
        var menu = new ContextMenuStrip();
        var startupItem = new ToolStripMenuItem("Open at Startup")
        {
            Checked = _startupManager.IsEnabled,
            CheckOnClick = false
        };
        startupItem.Click += (_, _) => ToggleStartup(startupItem);

        menu.Items.Add("Show Insert", null, (_, _) => _overlay.ShowOverlay());
        menu.Items.Add($"Change Hotkey... ({_hotkeySettings.Binding.DisplayText})", null, (_, _) => ChangeHotkey());
        menu.Items.Add(startupItem);
        menu.Items.Add(new ToolStripSeparator());
        menu.Items.Add("Quit", null, (_, _) => ExitThread());
        return menu;
    }

    private GlobalHotkey RegisterHotkey(HotkeyBinding binding)
        => new(_overlay.Toggle, binding.Key, binding.Modifiers);

    private void ChangeHotkey()
    {
        var binding = HotkeyRecorderForm.Capture(_hotkeySettings.Binding);
        if (binding is null)
        {
            return;
        }

        _hotkey.Dispose();
        try
        {
            _hotkeySettings = new HotkeySettings(binding);
            _hotkeyStore.Save(_hotkeySettings);
            _hotkey = RegisterHotkey(binding);
            _notifyIcon.ContextMenuStrip = BuildMenu();
        }
        catch
        {
            _hotkey = RegisterHotkey(_hotkeySettings.Binding);
            MessageBox.Show(
                "That shortcut could not be registered. Choose a different combination.",
                "Insert",
                MessageBoxButtons.OK,
                MessageBoxIcon.Warning
            );
        }
    }

    private void ToggleStartup(ToolStripMenuItem item)
    {
        var nextEnabled = !item.Checked;
        try
        {
            _startupManager.SetEnabled(nextEnabled);
            item.Checked = nextEnabled;
        }
        catch (Exception ex)
        {
            MessageBox.Show(
                ex.Message,
                "Insert",
                MessageBoxButtons.OK,
                MessageBoxIcon.Warning
            );
        }
    }
}
