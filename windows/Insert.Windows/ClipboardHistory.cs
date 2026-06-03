using System;
using System.Collections.Generic;
using System.Drawing;
using System.IO;
using System.Text.Json;
using System.Windows.Forms;

namespace Insert.Windows;

sealed class ClipboardHistory : IDisposable
{
    private readonly Timer _timer;
    private readonly string _storagePath;
    private readonly JsonSerializerOptions _jsonOptions = new() { WriteIndented = true };
    private ClipboardSnapshot? _lastSnapshot;
    private readonly List<ClipboardEntry> _entries;

    public event EventHandler? Changed;

    public IReadOnlyList<ClipboardEntry> Entries => _entries;

    public ClipboardHistory()
    {
        var appData = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
        var dir = Path.Combine(appData, "Insert");
        Directory.CreateDirectory(dir);
        _storagePath = Path.Combine(dir, "ClipboardHistory.json");
        _entries = Load();
        _timer = new Timer { Interval = 500 };
        _timer.Tick += (_, _) => PollClipboard();
        _timer.Start();
    }

    public void Delete(ClipboardEntry entry)
    {
        _entries.RemoveAll(e => e.Id == entry.Id);
        Save();
        Changed?.Invoke(this, EventArgs.Empty);
    }

    public void CopyBack(ClipboardEntry entry)
    {
        try
        {
            Clipboard.SetDataObject(entry.Payload.ToDataObject(), true);
            _lastSnapshot = ClipboardSnapshot.FromClipboard();
        }
        catch
        {
            // Clipboard can be temporarily unavailable if another app holds it.
        }
    }

    private void PollClipboard()
    {
        try
        {
            if (!Clipboard.ContainsText() &&
                !Clipboard.ContainsImage() &&
                !Clipboard.ContainsFileDropList() &&
                !Clipboard.ContainsData(DataFormats.Html) &&
                !Clipboard.ContainsData(DataFormats.Rtf) &&
                !Clipboard.ContainsData(DataFormats.UnicodeText) &&
                !Clipboard.ContainsData(DataFormats.Text) &&
                !Clipboard.ContainsData(DataFormats.Bitmap) &&
                !Clipboard.ContainsData(DataFormats.Dib) &&
                !Clipboard.ContainsData(DataFormats.Serializable))
            {
                return;
            }

            var snapshot = ClipboardSnapshot.FromClipboard();
            if (snapshot is null)
            {
                return;
            }

            if (_lastSnapshot is not null && snapshot.Equals(_lastSnapshot))
            {
                return;
            }

            _lastSnapshot = snapshot;

            var entry = snapshot.ToEntry();
            if (_entries.Count > 0 && _entries[0].SearchText == entry.SearchText && _entries[0].Kind == entry.Kind)
            {
                return;
            }

            _entries.RemoveAll(e => e.SearchText == entry.SearchText && e.Kind == entry.Kind);
            _entries.Insert(0, entry);
            if (_entries.Count > 100)
            {
                _entries.RemoveRange(100, _entries.Count - 100);
            }

            Save();
            Changed?.Invoke(this, EventArgs.Empty);
        }
        catch
        {
            // Ignore transient clipboard access failures.
        }
    }

    private List<ClipboardEntry> Load()
    {
        try
        {
            if (!File.Exists(_storagePath))
            {
                return [];
            }

            var json = File.ReadAllText(_storagePath);
            return JsonSerializer.Deserialize<List<ClipboardEntry>>(json, _jsonOptions) ?? [];
        }
        catch
        {
            return [];
        }
    }

    private void Save()
    {
        try
        {
            File.WriteAllText(_storagePath, JsonSerializer.Serialize(_entries, _jsonOptions));
        }
        catch
        {
        }
    }

    public void Dispose()
    {
        _timer.Stop();
        _timer.Dispose();
        Save();
    }
}

sealed record ClipboardSnapshot(
    ClipboardKind Kind,
    string Title,
    string Preview,
    string SearchText,
    ClipboardPayload Payload,
    string SourceApp
)
{
    public static ClipboardSnapshot? FromClipboard()
    {
        if (Clipboard.ContainsFileDropList())
        {
            var list = Clipboard.GetFileDropList();
            var files = new List<string>();
            foreach (var item in list)
            {
                files.Add(item);
            }

            if (files.Count == 0) return null;

            var title = files.Count == 1 ? Path.GetFileName(files[0]) : $"{files.Count} Files";
            var preview = files.Count == 1 ? files[0] : string.Join(Environment.NewLine, files.GetRange(0, Math.Min(3, files.Count)));
            return new ClipboardSnapshot(
                ClipboardKind.Files,
                title,
                preview,
                string.Join(" ", files),
                new ClipboardPayload(null, null, null, null, files, null, null, null, null, null, null),
                "Clipboard"
            );
        }

        if (Clipboard.ContainsImage() && Clipboard.GetImage() is Image image)
        {
            using var bitmap = new Bitmap(image);
            using var ms = new MemoryStream();
            bitmap.Save(ms, System.Drawing.Imaging.ImageFormat.Png);
            var bytes = ms.ToArray();
            return new ClipboardSnapshot(
                ClipboardKind.Image,
                "Image",
                $"{bitmap.Width} x {bitmap.Height}",
                $"image {bitmap.Width} {bitmap.Height}",
                new ClipboardPayload(null, null, null, null, null, Convert.ToBase64String(bytes), bitmap.Width, bitmap.Height, null, null, null),
                "Clipboard"
            );
        }

        if (Clipboard.ContainsData(DataFormats.Html))
        {
            var html = Clipboard.GetData(DataFormats.Html)?.ToString() ?? "";
            if (!string.IsNullOrWhiteSpace(html))
            {
                var preview = HtmlStripper.Strip(html);
                return new ClipboardSnapshot(
                    ClipboardKind.Html,
                    "HTML",
                    preview,
                    html,
                    new ClipboardPayload(null, html, null, null, null, null, null, null, null, null, null),
                    "Clipboard"
                );
            }
        }

        if (Clipboard.ContainsData(DataFormats.Rtf))
        {
            var rtf = Clipboard.GetData(DataFormats.Rtf)?.ToString() ?? "";
            if (!string.IsNullOrWhiteSpace(rtf))
            {
                return new ClipboardSnapshot(
                    ClipboardKind.Rtf,
                    "Rich Text",
                    rtf[..Math.Min(120, rtf.Length)],
                    rtf,
                    new ClipboardPayload(null, null, rtf, null, null, null, null, null, null, null, null),
                    "Clipboard"
                );
            }
        }

        if (Clipboard.ContainsText(TextDataFormat.UnicodeText) || Clipboard.ContainsText())
        {
            var text = Clipboard.GetText(TextDataFormat.UnicodeText);
            if (!string.IsNullOrWhiteSpace(text))
            {
                if (Uri.TryCreate(text.Trim(), UriKind.Absolute, out var uri) &&
                    (uri.Scheme is "http" or "https" or "file" or "mailto"))
                {
                    return new ClipboardSnapshot(
                        ClipboardKind.Url,
                        uri.Host.Length > 0 ? uri.Host : "URL",
                        text.Trim(),
                        text,
                        new ClipboardPayload(text, null, null, text, null, null, null, null, null, null, null),
                        "Clipboard"
                    );
                }

                return new ClipboardSnapshot(
                    ClipboardKind.Text,
                    TitleForText(text),
                    text.Trim(),
                    text,
                    new ClipboardPayload(text, null, null, null, null, null, null, null, null, null, null),
                    "Clipboard"
                );
            }
        }

        return TryCreateGenericSnapshot();
    }

    public ClipboardEntry ToEntry()
        => new(Guid.NewGuid(), DateTimeOffset.Now, SourceApp, Kind, Title, Preview, SearchText, Payload);

    private static string TitleForText(string text)
    {
        var firstLine = text.Split(['\r', '\n'], StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries);
        return firstLine.Length > 0 ? firstLine[0] : "Text";
    }

    private static ClipboardSnapshot? TryCreateGenericSnapshot()
    {
        var dataObject = Clipboard.GetDataObject();
        if (dataObject is null)
        {
            return null;
        }

        var formats = dataObject.GetFormats();
        var extraRepresentations = new List<ClipboardRepresentation>();
        string? title = null;
        string? preview = null;
        var kind = ClipboardKind.Data;

        foreach (var format in formats)
        {
            if (ShouldSkipGenericFormat(format))
            {
                continue;
            }

            var value = dataObject.GetData(format);
            if (value is null)
            {
                continue;
            }

            switch (value)
            {
                case System.Drawing.Color color:
                    extraRepresentations.Add(new ClipboardRepresentation(format, color.Name, null));
                    title ??= "Color";
                    preview ??= color.Name;
                    kind = ClipboardKind.Color;
                    break;
                case string text when !string.IsNullOrWhiteSpace(text):
                    extraRepresentations.Add(new ClipboardRepresentation(format, text, null));
                    title ??= FormatTitle(format, text);
                    preview ??= text.Length > 120 ? text[..120] : text;
                    kind = GuessKind(format, kind);
                    break;
                case Stream stream:
                    using (var ms = new MemoryStream())
                    {
                        stream.CopyTo(ms);
                        var bytes = ms.ToArray();
                        if (bytes.Length > 0)
                        {
                            extraRepresentations.Add(new ClipboardRepresentation(format, null, Convert.ToBase64String(bytes)));
                            title ??= FormatTitle(format, $"{bytes.Length} bytes");
                            preview ??= $"{bytes.Length} bytes";
                            kind = GuessKind(format, kind);
                        }
                    }
                    break;
                case byte[] bytes when bytes.Length > 0:
                    extraRepresentations.Add(new ClipboardRepresentation(format, null, Convert.ToBase64String(bytes)));
                    title ??= FormatTitle(format, $"{bytes.Length} bytes");
                    preview ??= $"{bytes.Length} bytes";
                    kind = GuessKind(format, kind);
                    break;
            }
        }

        if (extraRepresentations.Count == 0)
        {
            return null;
        }

        title ??= "Clipboard Data";
        preview ??= "Data";
        return new ClipboardSnapshot(
            kind,
            title,
            preview,
            string.Join(" ", extraRepresentations.Select(r => r.Text ?? r.Format)),
            new ClipboardPayload(null, null, null, null, null, null, null, null, null, null, extraRepresentations),
            "Clipboard"
        );
    }

    private static bool ShouldSkipGenericFormat(string format)
        => format is DataFormats.Text or DataFormats.UnicodeText or DataFormats.Html or DataFormats.Rtf or DataFormats.Bitmap or DataFormats.Dib or DataFormats.FileDrop;

    private static ClipboardKind GuessKind(string format, ClipboardKind current)
    {
        var lower = format.ToLowerInvariant();
        if (lower.Contains("url") || lower.Contains("uri") || lower.Contains("link"))
        {
            return ClipboardKind.Url;
        }

        if (lower.Contains("color") || lower.Contains("colour"))
        {
            return ClipboardKind.Color;
        }

        if (lower.Contains("wave") || lower.Contains("audio"))
        {
            return ClipboardKind.Audio;
        }

        if (lower.Contains("video") || lower.Contains("movie"))
        {
            return ClipboardKind.Video;
        }

        return current;
    }

    private static string FormatTitle(string format, string fallback)
        => string.IsNullOrWhiteSpace(format) ? fallback : format;
}

static class HtmlStripper
{
    public static string Strip(string html)
    {
        var plain = System.Text.RegularExpressions.Regex.Replace(html, "<[^>]+>", " ");
        plain = System.Text.RegularExpressions.Regex.Replace(plain, "\\s+", " ").Trim();
        return plain.Length > 0 ? plain : "HTML";
    }
}
