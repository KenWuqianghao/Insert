using System.Diagnostics;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Runtime.Versioning;
using Insert.Core;
using static Insert.Platform.Windows.Native;

namespace Insert.Platform.Windows;

[SupportedOSPlatform("windows")]
internal sealed unsafe class WinApps : IAppService
{
    private const int AppIconSide = 64;
    private readonly Dictionary<string, SourceApp?> _apps = new(StringComparer.OrdinalIgnoreCase);

    public WindowHandle ForegroundWindow() => new(GetForegroundWindow());

    public SourceApp? ForegroundApp()
    {
        var window = GetForegroundWindow();
        if (window == 0) return null;
        GetWindowThreadProcessId(window, out var process);
        var path = ProcessPath(process);

        // A Store app runs in its own process inside a frame window of ApplicationFrameHost.
        if (path is not null && Path.GetFileName(path).Equals("ApplicationFrameHost.exe", StringComparison.OrdinalIgnoreCase))
        {
            var hosted = HostedProcess(window, process);
            if (hosted != 0) path = ProcessPath(hosted) ?? path;
        }
        return path is null ? null : Describe(path);
    }

    public SourceApp? Describe(string executablePath)
    {
        if (_apps.TryGetValue(executablePath, out var cached)) return cached;
        SourceApp? app = null;
        try
        {
            var info = FileVersionInfo.GetVersionInfo(executablePath);
            var name = new[] { info.FileDescription, info.ProductName }
                .FirstOrDefault(text => !string.IsNullOrWhiteSpace(text))?.Trim()
                ?? Path.GetFileNameWithoutExtension(executablePath);
            app = new SourceApp(executablePath, name);
        }
        catch (FileNotFoundException)
        {
            app = new SourceApp(executablePath, Path.GetFileNameWithoutExtension(executablePath));
        }
        _apps[executablePath] = app;
        return app;
    }

    public IconPixels? AppIcon(SourceApp app)
    {
        nint icon = 0;
        uint id = 0;
        if (PrivateExtractIconsW(app.Id, 0, AppIconSide, AppIconSide, &icon, &id, 1, 0) == 1 && icon != 0)
        {
            try
            {
                return IconToPixels(icon, AppIconSide);
            }
            finally
            {
                DestroyIcon(icon);
            }
        }
        return FileIcon(app.Id);
    }

    /// <summary>The shell icon of a file at 48 × 48, from the system image list.</summary>
    public IconPixels? FileIcon(string path)
    {
        var info = new SHFILEINFOW();
        if (SHGetFileInfoW(path, 0, &info, (uint)sizeof(SHFILEINFOW), SHGFI_SYSICONINDEX) == 0) return null;

        var iid = new Guid("46EB5926-582E-4017-9FDF-E8998DAA0950");
        void* list = null;
        if (SHGetImageList(SHIL_EXTRALARGE, &iid, &list) != 0 || list is null) return null;
        try
        {
            // IImageList::GetIcon is slot 10 of the vtable.
            var getIcon = (delegate* unmanaged[Stdcall]<void*, int, int, nint*, int>)(*(void***)list)[10];
            nint icon = 0;
            if (getIcon(list, info.iIcon, ILD_TRANSPARENT, &icon) != 0 || icon == 0) return null;
            try
            {
                return IconToPixels(icon, 48);
            }
            finally
            {
                DestroyIcon(icon);
            }
        }
        finally
        {
            var release = (delegate* unmanaged[Stdcall]<void*, uint>)(*(void***)list)[2];
            release(list);
        }
    }

    private static string? ProcessPath(uint processId)
    {
        var process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, processId);
        if (process == 0) return null;
        try
        {
            var buffer = stackalloc char[1024];
            uint size = 1024;
            return QueryFullProcessImageNameW(process, 0, buffer, ref size) ? new string(buffer, 0, (int)size) : null;
        }
        finally
        {
            CloseHandle(process);
        }
    }

    private static uint s_frameProcess;
    private static uint s_hostedProcess;

    private static uint HostedProcess(nint frame, uint frameProcess)
    {
        s_frameProcess = frameProcess;
        s_hostedProcess = 0;
        EnumChildWindows(frame, &FindHosted, 0);
        return s_hostedProcess;
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvStdcall)])]
    private static int FindHosted(nint child, nint parameter)
    {
        GetWindowThreadProcessId(child, out var process);
        if (process == s_frameProcess) return 1;
        s_hostedProcess = process;
        return 0;
    }

    /// <summary>Draws the icon into a 32-bit bitmap and returns straight-alpha pixels.</summary>
    private static IconPixels? IconToPixels(nint icon, int side)
    {
        var header = new BITMAPINFOHEADER
        {
            biSize = (uint)sizeof(BITMAPINFOHEADER),
            biWidth = side,
            biHeight = -side,
            biPlanes = 1,
            biBitCount = 32,
        };
        var dc = CreateCompatibleDC(0);
        void* bits = null;
        var bitmap = CreateDIBSection(dc, &header, 0, &bits, 0, 0);
        if (bitmap == 0 || bits is null)
        {
            DeleteDC(dc);
            return null;
        }
        var old = SelectObject(dc, bitmap);
        try
        {
            if (!DrawIconEx(dc, 0, 0, icon, side, side, 0, 0, DI_NORMAL)) return null;
            var pixels = new ReadOnlySpan<byte>(bits, side * side * 4).ToArray();

            var hasAlpha = false;
            for (var index = 3; index < pixels.Length; index += 4) hasAlpha |= pixels[index] != 0;
            for (var index = 0; index < pixels.Length; index += 4)
            {
                if (!hasAlpha)
                {
                    // An old icon without alpha draws its mask as black. Treat other pixels as opaque.
                    pixels[index + 3] = (byte)(pixels[index] | pixels[index + 1] | pixels[index + 2]) == 0 ? (byte)0 : (byte)255;
                    continue;
                }
                var alpha = pixels[index + 3];
                if (alpha is 0 or 255) continue;
                for (var channel = 0; channel < 3; channel++)
                    pixels[index + channel] = (byte)Math.Min(255, pixels[index + channel] * 255 / alpha);
            }
            return new IconPixels(pixels, side, side);
        }
        finally
        {
            SelectObject(dc, old);
            DeleteObject(bitmap);
            DeleteDC(dc);
        }
    }
}
