using System;
using System.Drawing;
using System.Media;
using System.Windows.Forms;

namespace Insert.Windows;

sealed class HotkeyRecorderForm : Form
{
    private readonly Label _prompt;
    private HotkeyBinding? _result;

    public HotkeyRecorderForm(HotkeyBinding current)
    {
        FormBorderStyle = FormBorderStyle.FixedDialog;
        StartPosition = FormStartPosition.CenterScreen;
        MinimizeBox = false;
        MaximizeBox = false;
        ShowInTaskbar = false;
        KeyPreview = true;
        Width = 420;
        Height = 180;
        Text = "Change Hotkey";

        _prompt = new Label
        {
            Dock = DockStyle.Fill,
            TextAlign = ContentAlignment.MiddleCenter,
            Font = new Font(Font.FontFamily, 10, FontStyle.Regular),
            Text = $"Current: {current.DisplayText}\nPress a new shortcut with Ctrl, Alt, or Shift. Esc cancels.",
        };

        Controls.Add(_prompt);
        KeyDown += HandleKeyDown;
    }

    public static HotkeyBinding? Capture(HotkeyBinding current)
    {
        using var form = new HotkeyRecorderForm(current);
        return form.ShowDialog() == DialogResult.OK ? form._result : null;
    }

    private void HandleKeyDown(object? sender, KeyEventArgs e)
    {
        if (e.KeyCode == Keys.Escape)
        {
            DialogResult = DialogResult.Cancel;
            Close();
            return;
        }

        var modifiers = HotkeyModifiers.None;
        if (e.Control) modifiers |= HotkeyModifiers.Control;
        if (e.Alt) modifiers |= HotkeyModifiers.Alt;
        if (e.Shift) modifiers |= HotkeyModifiers.Shift;
        if (modifiers == HotkeyModifiers.None)
        {
            SystemSounds.Beep.Play();
            return;
        }

        if (e.KeyCode is Keys.ControlKey or Keys.Menu or Keys.ShiftKey or Keys.LWin or Keys.RWin)
        {
            return;
        }

        _result = new HotkeyBinding(e.KeyCode, modifiers);
        DialogResult = DialogResult.OK;
        Close();
    }
}
