using Insert.Core;
using Insert.Platform;

namespace Insert.Services;

/// <summary>Saves each new copy of another app in the library.</summary>
public sealed class ClipboardMonitor(IPlatform platform, ClipLibrary library, Preferences preferences, IImageCodec images)
{
    public void Start() => platform.Clipboard.Changed += Capture;

    private void Capture()
    {
        if (preferences.Data.CapturePaused) return;
        var source = platform.Apps.ForegroundApp();
        if (source is not null && preferences.Data.IgnoredApps.Any(app => source.SameId(app.Id)))
        {
            Log.Info($"capture: skipped, {source.Name} is ignored");
            return;
        }

        var payload = platform.Clipboard.Read();
        if (payload is null)
        {
            Log.Info("capture: skipped, the copy is private or has no stored format");
            return;
        }
        if (ClipCapture.Build(payload, source, images, DateTimeOffset.Now) is not { } capture)
        {
            Log.Info($"capture: skipped, no card for formats {string.Join(", ", payload.Representations.Select(r => r.Format))}");
            return;
        }
        try
        {
            library.Insert(capture);
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            Log.Info($"capture: skipped, the clip could not be saved: {error.Message}");
            return;
        }
        Log.Info($"capture: {capture.Clip.Kind} from {source?.Name ?? "unknown app"}, formats {string.Join(", ", payload.Representations.Select(r => r.Format))}");
    }
}
