using System;
using System.Drawing;
using System.IO;
using System.Linq;
using System.Windows.Forms;

namespace Insert.Windows;

sealed class OverlayForm : Form
{
    private readonly ClipboardHistory _history;
    private readonly TextBox _searchBox;
    private readonly ListView _list;
    private readonly Label _emptyLabel;
    private readonly Button _closeButton;
    private readonly Button _deleteButton;
    private bool _isOpen;

    public OverlayForm(ClipboardHistory history)
    {
        _history = history;

        FormBorderStyle = FormBorderStyle.None;
        StartPosition = FormStartPosition.Manual;
        ShowInTaskbar = false;
        TopMost = true;
        BackColor = Color.FromArgb(28, 28, 30);
        Padding = new Padding(18);
        KeyPreview = true;
        Width = 1200;
        Height = 360;

        var root = new Panel
        {
            Dock = DockStyle.Fill,
            BackColor = Color.FromArgb(42, 42, 46),
            Padding = new Padding(18),
        };

        _searchBox = new TextBox
        {
            PlaceholderText = "Search",
            BorderStyle = BorderStyle.FixedSingle,
            Dock = DockStyle.Top,
            Height = 32,
        };
        _searchBox.TextChanged += (_, _) => RefreshHistory();

        _closeButton = new Button { Text = "X", Width = 32, Height = 32, Dock = DockStyle.Right };
        _closeButton.Click += (_, _) => HideOverlay();

        _deleteButton = new Button { Text = "Del", Width = 48, Height = 32, Dock = DockStyle.Right };
        _deleteButton.Click += (_, _) => DeleteSelected();

        var header = new Panel { Dock = DockStyle.Top, Height = 36 };
        header.Controls.Add(_closeButton);
        header.Controls.Add(_deleteButton);
        header.Controls.Add(_searchBox);

        _list = new ListView
        {
            Dock = DockStyle.Fill,
            View = View.Tile,
            MultiSelect = false,
            HideSelection = false,
            FullRowSelect = true,
            OwnerDraw = true,
        };
        _list.DrawItem += DrawItem;
        _list.SelectedIndexChanged += (_, _) => _list.Invalidate();
        _list.DoubleClick += (_, _) => CopySelected();

        _emptyLabel = new Label
        {
            Dock = DockStyle.Fill,
            Text = "Clipboard Empty",
            TextAlign = ContentAlignment.MiddleCenter,
            ForeColor = Color.Gainsboro,
            Visible = false,
        };

        root.Controls.Add(_emptyLabel);
        root.Controls.Add(_list);
        root.Controls.Add(header);
        Controls.Add(root);

        Load += (_, _) => RefreshHistory();
        Deactivate += (_, _) => HideOverlay();
        KeyDown += OverlayForm_KeyDown;
    }

    public void Toggle()
    {
        if (_isOpen)
        {
            HideOverlay();
        }
        else
        {
            ShowOverlay();
        }
    }

    public void ShowOverlay()
    {
        var screen = Screen.PrimaryScreen?.WorkingArea ?? new Rectangle(0, 0, 1400, 900);
        Width = Math.Min(Math.Max(screen.Width - 48, 900), 1500);
        Height = Math.Min(Math.Max((int)(screen.Height * 0.34), 300), 420);
        Left = screen.Left + (screen.Width - Width) / 2;
        Top = screen.Bottom - Height - 24;

        Show();
        Activate();
        BringToFront();
        _searchBox.Focus();
        _isOpen = true;
        RefreshHistory();
    }

    public void HideOverlay()
    {
        Hide();
        _isOpen = false;
    }

    public void RefreshHistory()
    {
        var query = _searchBox.Text.Trim();
        var selectedId = _list.SelectedItems.Count > 0 && _list.SelectedItems[0].Tag is ClipboardEntry selectedEntry
            ? selectedEntry.Id
            : (Guid?)null;
        var items = string.IsNullOrWhiteSpace(query)
            ? _history.Entries
            : _history.Entries.Where(e =>
                e.SearchText.Contains(query, StringComparison.OrdinalIgnoreCase) ||
                e.Title.Contains(query, StringComparison.OrdinalIgnoreCase) ||
                e.Preview.Contains(query, StringComparison.OrdinalIgnoreCase) ||
                e.Kind.ToString().Contains(query, StringComparison.OrdinalIgnoreCase) ||
                e.SourceApp.Contains(query, StringComparison.OrdinalIgnoreCase)
            ).ToList();

        _list.BeginUpdate();
        _list.Items.Clear();

        foreach (var item in items)
        {
            var listViewItem = new ListViewItem
            {
                Text = item.Title,
                Tag = item
            };
            listViewItem.SubItems.Add(item.Preview);
            listViewItem.SubItems.Add(item.Kind.ToString());
            _list.Items.Add(listViewItem);
        }

        _list.EndUpdate();
        _emptyLabel.Visible = _list.Items.Count == 0;

        if (_list.Items.Count > 0)
        {
            foreach (ListViewItem item in _list.Items)
            {
                item.Selected = false;
            }

            var index = 0;
            if (selectedId.HasValue)
            {
                for (var i = 0; i < _list.Items.Count; i++)
                {
                    if (_list.Items[i].Tag is ClipboardEntry entry && entry.Id == selectedId.Value)
                    {
                        index = i;
                        break;
                    }
                }
            }

            _list.Items[index].Selected = true;
            _list.Items[index].Focused = true;
            _list.EnsureVisible(index);
        }
    }

    private void CopySelected()
    {
        if (_list.SelectedItems.Count == 0)
        {
            return;
        }

        if (_list.SelectedItems[0].Tag is ClipboardEntry entry)
        {
            _history.CopyBack(entry);
            HideOverlay();
        }
    }

    private void DeleteSelected()
    {
        if (_list.SelectedItems.Count == 0)
        {
            return;
        }

        if (_list.SelectedItems[0].Tag is ClipboardEntry entry)
        {
            _history.Delete(entry);
            RefreshHistory();
        }
    }

    private void OverlayForm_KeyDown(object? sender, KeyEventArgs e)
    {
        if (e.KeyCode == Keys.Escape)
        {
            HideOverlay();
            e.Handled = true;
            return;
        }

        if (e.KeyCode == Keys.Enter)
        {
            CopySelected();
            e.Handled = true;
            return;
        }

        if (e.KeyCode == Keys.Back || e.KeyCode == Keys.Delete)
        {
            DeleteSelected();
            e.Handled = true;
            return;
        }

        if (e.KeyCode == Keys.Left || e.KeyCode == Keys.Up)
        {
            MoveSelection(-1);
            e.Handled = true;
            return;
        }

        if (e.KeyCode == Keys.Right || e.KeyCode == Keys.Down)
        {
            MoveSelection(1);
            e.Handled = true;
            return;
        }
    }

    private void MoveSelection(int delta)
    {
        if (_list.Items.Count == 0)
        {
            return;
        }

        var selectedIndex = _list.SelectedIndices.Count > 0 ? _list.SelectedIndices[0] : 0;
        var next = Math.Clamp(selectedIndex + delta, 0, _list.Items.Count - 1);
        _list.Items[next].Selected = true;
        _list.Items[next].Focused = true;
        _list.EnsureVisible(next);
    }

    private void DrawItem(object? sender, DrawListViewItemEventArgs e)
    {
        var entry = e.Item.Tag as ClipboardEntry;
        if (entry is null)
        {
            return;
        }

        using var titleBrush = new SolidBrush(Color.WhiteSmoke);
        using var previewBrush = new SolidBrush(Color.LightGray);
        using var kindBrush = new SolidBrush(Color.Silver);
        using var iconBrush = new SolidBrush(Color.FromArgb(255, 70, 70, 74));
        var bounds = e.Bounds;
        var selected = e.Item.Selected;
        var bg = selected ? Color.FromArgb(74, 74, 82) : Color.FromArgb(45, 45, 49);
        using (var fill = new SolidBrush(bg))
        {
            e.Graphics.FillRectangle(fill, bounds);
        }

        var iconRect = new Rectangle(bounds.Left + 12, bounds.Top + 12, 36, 36);
        e.Graphics.FillEllipse(iconBrush, iconRect);
        using var boldFont = new Font(Font, FontStyle.Bold);
        e.Graphics.DrawString(entry.Kind.ToString().Substring(0, 1), Font, previewBrush, iconRect.Left + 10, iconRect.Top + 8);
        e.Graphics.DrawString(entry.Title, boldFont, titleBrush, bounds.Left + 58, bounds.Top + 12);
        if (!string.IsNullOrEmpty(entry.Payload.ImageBase64))
        {
            DrawImagePreview(e.Graphics, entry.Payload.ImageBase64, new Rectangle(bounds.Left + 58, bounds.Top + 34, 180, 88));
        }
        else
        {
            e.Graphics.DrawString(entry.Preview, Font, previewBrush, bounds.Left + 58, bounds.Top + 34);
        }
        e.Graphics.DrawString(entry.SourceApp, Font, kindBrush, bounds.Right - 130, bounds.Top + 12);

        if (selected)
        {
            using var pen = new Pen(Color.FromArgb(220, 255, 80, 80), 2);
            e.Graphics.DrawRectangle(pen, bounds.Left, bounds.Top, bounds.Width - 1, bounds.Height - 1);
        }
    }

    private static void DrawImagePreview(Graphics graphics, string base64, Rectangle bounds)
    {
        try
        {
            var bytes = Convert.FromBase64String(base64);
            using var ms = new MemoryStream(bytes);
            using var image = Image.FromStream(ms);
            graphics.DrawImage(image, bounds);
        }
        catch
        {
        }
    }
}
