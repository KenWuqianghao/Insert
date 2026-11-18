namespace Insert.Services;

/// <summary>Writes one line for each event to <c>insert.log</c> in the data folder and to the standard error.</summary>
public static class Log
{
    private const long MaxBytes = 1_000_000;
    private static readonly Lock Gate = new();
    private static string? s_path;

    public static void Start(string folder)
    {
        Directory.CreateDirectory(folder);
        s_path = Path.Combine(folder, "insert.log");
        try
        {
            if (File.Exists(s_path) && new FileInfo(s_path).Length > MaxBytes) File.Move(s_path, s_path + ".1", overwrite: true);
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
        }
    }

    public static void Info(string message)
    {
        var line = $"{DateTimeOffset.Now:yyyy-MM-dd HH:mm:ss.fff} {message}";
        lock (Gate)
        {
            // The hooks and the message window log from native callbacks, so a failed write must not throw.
            Try(() => Console.Error.WriteLine(line));
            if (s_path is { } path) Try(() => File.AppendAllText(path, line + Environment.NewLine));
        }
    }

    private static void Try(Action write)
    {
        try
        {
            write();
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException or ObjectDisposedException)
        {
        }
    }
}
