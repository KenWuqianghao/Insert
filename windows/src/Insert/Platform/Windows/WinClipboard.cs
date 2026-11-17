using System.Buffers.Binary;
using System.Runtime.Versioning;
using System.Text;
using Insert.Core;
using static Insert.Platform.Windows.Native;

namespace Insert.Platform.Windows;

[SupportedOSPlatform("windows")]
internal sealed unsafe class WinClipboard : IClipboardService, IDisposable
{
    private nint _window;
    private uint _ownSequence;

    public event Action? Changed;

    public void Attach(nint window)
    {
        _window = window;
        AddClipboardFormatListener(window);
    }

    public void OnClipboardUpdate()
    {
        // Insert ignores the change that its own write caused.
        if (GetClipboardSequenceNumber() == _ownSequence) return;
        Changed?.Invoke();
    }

    public ClipPayload? Read()
    {
        if (!Open()) return null;
        try
        {
            var available = new Dictionary<string, uint>();
            for (var format = EnumClipboardFormats(0); format != 0; format = EnumClipboardFormats(format))
            {
                if (FormatName(format) is { } name) available.TryAdd(name, format);
            }

            if (ClipCapture.IsPrivate(available.Keys, name => ReadData(available[name])))
                return null;

            var representations = new List<Representation>();
            long total = 0;
            foreach (var name in ClipFormats.Stored)
            {
                if (!available.TryGetValue(name, out var format) || ReadData(format) is not { Length: > 0 } raw) continue;
                var data = Normalize(name, raw);
                if (data is not { Length: > 0 }) continue;
                total += data.Length;
                if (total > ClipCapture.MaxByteCount) return null;
                representations.Add(new Representation(name, data));
            }
            // A DIB and a DIBV5 are two forms of the same picture. Windows makes the second from the first.
            if (representations.Any(r => r.Format == ClipFormats.Dib))
                representations.RemoveAll(r => r.Format == ClipFormats.DibV5);
            return representations.Count == 0 ? null : new ClipPayload(representations);
        }
        finally
        {
            CloseClipboard();
        }
    }

    public bool Write(ClipPayload payload)
    {
        if (!Open()) return false;
        try
        {
            EmptyClipboard();
            foreach (var representation in payload.Representations)
            {
                var format = FormatId(representation.Format);
                var data = Denormalize(representation.Format, representation.Data);
                SetData(format, data);
            }
            if (payload.Has(ClipFormats.FileDrop))
            {
                // File Explorer reads this format to copy the files, not move them.
                SetData(RegisterClipboardFormatW("Preferred DropEffect"), BitConverter.GetBytes(1));
            }
        }
        finally
        {
            CloseClipboard();
            _ownSequence = GetClipboardSequenceNumber();
        }
        return true;
    }

    /// <summary>Another app can have the clipboard open for a short time.</summary>
    private bool Open()
    {
        for (var attempt = 0; attempt < 10; attempt++)
        {
            if (OpenClipboard(_window)) return true;
            Thread.Sleep(15);
        }
        return false;
    }

    private static string? FormatName(uint format)
    {
        switch (format)
        {
            case CF_UNICODETEXT: return ClipFormats.UnicodeText;
            case CF_HDROP: return ClipFormats.FileDrop;
            case CF_DIB: return ClipFormats.Dib;
            case CF_DIBV5: return ClipFormats.DibV5;
            case < 0xC000: return null;
        }
        var buffer = stackalloc char[256];
        var length = GetClipboardFormatNameW(format, buffer, 256);
        return length > 0 ? new string(buffer, 0, length) : null;
    }

    private static uint FormatId(string name) => name switch
    {
        ClipFormats.UnicodeText => CF_UNICODETEXT,
        ClipFormats.FileDrop => CF_HDROP,
        ClipFormats.Dib => CF_DIB,
        ClipFormats.DibV5 => CF_DIBV5,
        _ => RegisterClipboardFormatW(name),
    };

    private static byte[]? ReadData(uint format)
    {
        var handle = GetClipboardData(format);
        if (handle == 0) return null;
        var pointer = GlobalLock(handle);
        if (pointer is null) return null;
        try
        {
            var size = (int)Math.Min((long)GlobalSize(handle), ClipCapture.MaxByteCount + 1);
            return new ReadOnlySpan<byte>(pointer, size).ToArray();
        }
        finally
        {
            GlobalUnlock(handle);
        }
    }

    private static void SetData(uint format, byte[] data)
    {
        var handle = GlobalAlloc(GMEM_MOVEABLE, (nuint)Math.Max(data.Length, 1));
        if (handle == 0) return;
        var pointer = GlobalLock(handle);
        data.CopyTo(new Span<byte>(pointer, data.Length));
        GlobalUnlock(handle);
        if (SetClipboardData(format, handle) == 0) GlobalFree(handle);
    }

    /// <summary>Text formats lose the null at the end. A file drop becomes UTF-8 lines.</summary>
    internal static byte[]? Normalize(string name, byte[] raw) => name switch
    {
        ClipFormats.UnicodeText or ClipFormats.Url => TrimUtf16(raw),
        ClipFormats.Html or ClipFormats.Rtf => raw.AsSpan(0, IndexOfNull(raw)).ToArray(),
        ClipFormats.FileDrop => FileDropPaths(raw) is { Count: > 0 } paths ? Encoding.UTF8.GetBytes(string.Join('\n', paths)) : null,
        _ => raw,
    };

    internal static byte[] Denormalize(string name, byte[] data) => name switch
    {
        ClipFormats.UnicodeText or ClipFormats.Url => [.. data, 0, 0],
        ClipFormats.Html or ClipFormats.Rtf => [.. data, 0],
        ClipFormats.FileDrop => DropFiles(Encoding.UTF8.GetString(data).Split('\n', StringSplitOptions.RemoveEmptyEntries)),
        _ => data,
    };

    private static byte[] TrimUtf16(byte[] raw)
    {
        var length = raw.Length & ~1;
        for (var index = 0; index + 1 < length; index += 2)
        {
            if (raw[index] == 0 && raw[index + 1] == 0) return raw[..index];
        }
        return raw[..length];
    }

    private static int IndexOfNull(byte[] raw)
    {
        var index = Array.IndexOf(raw, (byte)0);
        return index < 0 ? raw.Length : index;
    }

    /// <summary>DROPFILES: the offset of the file list, a point, two flags, then the null-separated paths.</summary>
    internal static IReadOnlyList<string> FileDropPaths(byte[] raw)
    {
        if (raw.Length < 20) return [];
        var offset = BinaryPrimitives.ReadInt32LittleEndian(raw);
        var isWide = BinaryPrimitives.ReadInt32LittleEndian(raw.AsSpan(16)) != 0;
        if (offset < 20 || offset >= raw.Length) return [];
        var text = isWide ? Encoding.Unicode.GetString(raw, offset, (raw.Length - offset) & ~1) : Encoding.Default.GetString(raw, offset, raw.Length - offset);
        var end = text.IndexOf("\0\0", StringComparison.Ordinal);
        return (end >= 0 ? text[..end] : text).Split('\0', StringSplitOptions.RemoveEmptyEntries);
    }

    internal static byte[] DropFiles(IReadOnlyList<string> paths)
    {
        var list = Encoding.Unicode.GetBytes(string.Concat(paths.Select(path => path + '\0')) + '\0');
        var bytes = new byte[20 + list.Length];
        BinaryPrimitives.WriteInt32LittleEndian(bytes, 20);
        BinaryPrimitives.WriteInt32LittleEndian(bytes.AsSpan(16), 1);
        list.CopyTo(bytes, 20);
        return bytes;
    }

    public void Dispose()
    {
        if (_window != 0) RemoveClipboardFormatListener(_window);
    }
}
