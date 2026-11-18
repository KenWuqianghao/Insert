using System.IO.Pipes;
using System.Security.Cryptography;
using System.Text;
using Avalonia.Threading;

namespace Insert.Services;

/// <summary>
/// One Insert runs for each data folder. A second start sends its command to the first one through a named pipe and exits.
/// </summary>
public sealed class CommandChannel : IDisposable
{
    private readonly FileStream _lock;
    private readonly string _pipeName;
    private readonly CancellationTokenSource _stop = new();

    private CommandChannel(FileStream lockFile, string pipeName)
    {
        _lock = lockFile;
        _pipeName = pipeName;
    }

    /// <summary>Returns the channel of the first instance, or null after the command went to the running instance.</summary>
    public static CommandChannel? Claim(string dataRoot, string command)
    {
        var id = Convert.ToHexStringLower(SHA256.HashData(Encoding.UTF8.GetBytes(Path.GetFullPath(dataRoot).ToLowerInvariant())))[..16];
        var pipeName = $"Insert-{id}";
        Directory.CreateDirectory(dataRoot);
        try
        {
            // The open file is the lock. The system releases it when the process ends, also after a crash.
            var lockFile = new FileStream(Path.Combine(dataRoot, "insert.lock"), FileMode.OpenOrCreate, FileAccess.ReadWrite, FileShare.None);
            return new CommandChannel(lockFile, pipeName);
        }
        catch (IOException)
        {
        }

        try
        {
            using var client = new NamedPipeClientStream(".", pipeName, PipeDirection.Out);
            client.Connect(2_000);
            using var writer = new StreamWriter(client);
            writer.WriteLine(command);
        }
        catch (Exception error) when (error is IOException or TimeoutException)
        {
            Console.Error.WriteLine($"Insert is running, but it did not take the command: {error.Message}");
        }
        return null;
    }

    public void Listen(Action<string> handle) => _ = Task.Run(async () =>
    {
        while (!_stop.IsCancellationRequested)
        {
            try
            {
                await using var server = new NamedPipeServerStream(_pipeName, PipeDirection.In, 1, PipeTransmissionMode.Byte, PipeOptions.Asynchronous);
                await server.WaitForConnectionAsync(_stop.Token);
                using var reader = new StreamReader(server);
                if (await reader.ReadLineAsync(_stop.Token) is { } line)
                    Dispatcher.UIThread.Post(() => handle(line.Trim()));
            }
            catch (OperationCanceledException)
            {
                return;
            }
            catch (Exception error) when (error is IOException or UnauthorizedAccessException)
            {
                Log.Info($"command channel: {error.Message}");
                try
                {
                    await Task.Delay(TimeSpan.FromSeconds(1), _stop.Token);
                }
                catch (OperationCanceledException)
                {
                    return;
                }
            }
        }
    });

    public void Dispose()
    {
        _stop.Cancel();
        _lock.Dispose();
    }
}
