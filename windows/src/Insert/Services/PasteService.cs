using Insert.Core;
using Insert.Platform;

namespace Insert.Services;

public sealed class PasteService(IPlatform platform, ClipLibrary library, Preferences preferences)
{
    /// <summary>Writes the clip to the clipboard, then pastes it into <paramref name="target"/>. Returns false when the payload file is missing.</summary>
    public bool Perform(PasteAction action, Clip clip, WindowHandle target)
    {
        if (library.Payload(clip.Id) is not { } payload) return false;

        var written = action == PasteAction.PastePlainText && payload.PlainText is { } text
            ? ClipPayload.FromText(text)
            : payload;
        if (!platform.Clipboard.Write(written))
        {
            Log.Info("paste: the clipboard is not available");
            return false;
        }
        library.MoveToFront(clip.Id);

        if (action != PasteAction.Copy && preferences.Data.PasteDirectly)
        {
            Log.Info($"paste: {action} of a {clip.Kind} clip into window 0x{target.Value:X}");
            _ = platform.Keyboard.Paste(target);
        }
        else
        {
            platform.Keyboard.Restore(target);
            Log.Info($"paste: copied a {clip.Kind} clip");
        }
        return true;
    }
}
