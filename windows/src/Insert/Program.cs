using Avalonia;
using Insert.Core;
using Insert.Services;

namespace Insert;

/// <summary>The command line. A second start of Insert sends <see cref="Command"/> to the running copy.</summary>
public sealed record Options(string DataRoot, string Command, bool Demo)
{
    public const string Usage = """
        Usage: Insert [--data <folder>] [--demo] [<command>]

        Commands for the running copy of Insert:
          --show            Open the tray. A second start without a command also opens the tray.
          --toggle          Open or close the tray.
          --ring [<n>]      Open the clip ring at the pointer, and move the pointer onto segment n.
          --settings        Open Settings.
          --quit            Quit Insert.
          --background      Start without a window. Launch at login uses this.

        Commands for tests on a system without Win32:
          --copy <text>     Put text on the fake clipboard as a copy from another app.
          --pointer <x,y>   Move the fake pointer, in physical pixels.
          --release         Release the fake hold key of the clip ring.

        --data <folder>     Keep clips and settings in this folder. The default is %LOCALAPPDATA%\Insert.
        --demo              Add sample clips when the library is empty.
        """;

    public static Options? Parse(string[] args)
    {
        var root = ClipStorage.DefaultRoot;
        var command = "start";
        var demo = false;
        for (var index = 0; index < args.Length; index++)
        {
            switch (args[index])
            {
                case "--data" when index + 1 < args.Length:
                    root = Path.GetFullPath(args[++index]);
                    break;
                case "--demo":
                    demo = true;
                    break;
                case "--show" or "--toggle" or "--settings" or "--quit" or "--background" or "--release":
                    command = args[index][2..];
                    break;
                case "--ring":
                    command = index + 1 < args.Length && int.TryParse(args[index + 1], out _) ? $"ring {args[++index]}" : "ring";
                    break;
                case "--copy" or "--pointer" when index + 1 < args.Length:
                    command = $"{args[index][2..]} {args[++index]}";
                    break;
                default:
                    Console.Error.WriteLine(Usage);
                    return null;
            }
        }
        return new Options(root, command, demo);
    }
}

public static class Program
{
    [STAThread]
    public static int Main(string[] args)
    {
        if (Options.Parse(args) is not { } options) return 2;
        using var channel = CommandChannel.Claim(options.DataRoot, options.Command);
        if (channel is null) return 0;

        App.Options = options;
        App.Channel = channel;
        return BuildAvaloniaApp().StartWithClassicDesktopLifetime(args, Avalonia.Controls.ShutdownMode.OnExplicitShutdown);
    }

    public static AppBuilder BuildAvaloniaApp() => AppBuilder.Configure<App>()
        .UsePlatformDetect()
        .With(new Win32PlatformOptions
        {
            // Wine and old drivers can lack Direct3D 11. Software rendering keeps Insert usable there.
            RenderingMode = [Win32RenderingMode.AngleEgl, Win32RenderingMode.Software],
        })
        .LogToTrace();
}
