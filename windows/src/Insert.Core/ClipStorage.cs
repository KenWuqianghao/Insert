using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace Insert.Core;

public sealed record LibraryIndex(IReadOnlyList<Clip> Clips, IReadOnlyList<Pinboard> Pinboards)
{
    public static readonly LibraryIndex Empty = new([], []);
}

/// <summary>
/// Files on disk: <c>index.json</c>, <c>payloads/&lt;clip id&gt;.bin</c>, <c>thumbnails/&lt;clip id&gt;.png</c>.
/// </summary>
public sealed class ClipStorage(string root)
{
    private static readonly byte[] PayloadMagic = "INSP"u8.ToArray();
    private const byte PayloadVersion = 1;

    public string Root { get; } = root;

    /// <summary><c>%LOCALAPPDATA%\Insert</c> on Windows.</summary>
    public static string DefaultRoot =>
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Insert");

    private string IndexPath => Path.Combine(Root, "index.json");
    private string PayloadsPath => Path.Combine(Root, "payloads");
    private string ThumbnailsPath => Path.Combine(Root, "thumbnails");
    private string PayloadPath(Guid id) => Path.Combine(PayloadsPath, $"{id:D}.bin");

    public string ThumbnailPath(Guid id) => Path.Combine(ThumbnailsPath, $"{id:D}.png");

    /// <summary>
    /// A file that Insert cannot read moves to <c>index.json.corrupt-&lt;time&gt;</c>.
    /// While such a file exists, the payloads can be the only copy of the clips, so the library keeps them.
    /// </summary>
    public bool HasCorruptBackup =>
        Directory.Exists(Root) && Directory.EnumerateFiles(Root, "index.json.corrupt-*").Any();

    public LibraryIndex LoadIndex()
    {
        if (!File.Exists(IndexPath)) return LibraryIndex.Empty;
        LibraryIndex? index;
        try
        {
            using var stream = File.OpenRead(IndexPath);
            index = JsonSerializer.Deserialize(stream, InsertJson.Default.LibraryIndex);
        }
        catch (Exception error) when (error is JsonException or NotSupportedException)
        {
            index = null;
        }
        if (index is not null && IsComplete(index)) return index;
        File.Move(IndexPath, $"{IndexPath}.corrupt-{DateTimeOffset.UtcNow.ToUnixTimeSeconds()}", overwrite: true);
        return LibraryIndex.Empty;
    }

    /// <summary>The serializer does not enforce non-null members, so a file such as <c>{"clips":null}</c> loads.</summary>
    private static bool IsComplete(LibraryIndex index) =>
        index.Clips is not null && index.Pinboards is not null
        && index.Clips.All(clip => clip is { Content: not null, ContentHash: not null })
        && index.Pinboards.All(pinboard => pinboard is { Name: not null });

    public void SaveIndex(LibraryIndex index) =>
        WriteAtomic(IndexPath, JsonSerializer.SerializeToUtf8Bytes(index, InsertJson.Default.LibraryIndex));

    public void Write(CapturedClip capture)
    {
        WriteAtomic(PayloadPath(capture.Clip.Id), EncodePayload(capture.Payload));
        if (capture.Thumbnail is { } thumbnail) WriteAtomic(ThumbnailPath(capture.Clip.Id), thumbnail);
    }

    public ClipPayload? LoadPayload(Guid id)
    {
        try
        {
            return DecodePayload(File.ReadAllBytes(PayloadPath(id)));
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            return null;
        }
    }

    public bool HasPayload(Guid id) => File.Exists(PayloadPath(id));

    public void RemoveFiles(Guid id)
    {
        TryDelete(PayloadPath(id));
        TryDelete(ThumbnailPath(id));
    }

    /// <summary>A crash between a file write and an index write can leave files that no clip owns.</summary>
    public void RemoveOrphans(IReadOnlySet<Guid> keep)
    {
        foreach (var directory in new[] { PayloadsPath, ThumbnailsPath })
        {
            if (!Directory.Exists(directory)) continue;
            foreach (var file in Directory.EnumerateFiles(directory))
            {
                var name = Path.GetFileNameWithoutExtension(file);
                if (!Guid.TryParse(name, out var id) || !keep.Contains(id)) TryDelete(file);
            }
        }
    }

    /// <summary>"INSP", version 1, the count, then the format name and the data of each representation, with 32-bit lengths.</summary>
    public static byte[] EncodePayload(ClipPayload payload)
    {
        using var stream = new MemoryStream();
        using var writer = new BinaryWriter(stream, Encoding.UTF8);
        writer.Write(PayloadMagic);
        writer.Write(PayloadVersion);
        writer.Write(payload.Representations.Count);
        foreach (var representation in payload.Representations)
        {
            var name = Encoding.UTF8.GetBytes(representation.Format);
            writer.Write(name.Length);
            writer.Write(name);
            writer.Write(representation.Data.Length);
            writer.Write(representation.Data);
        }
        writer.Flush();
        return stream.ToArray();
    }

    public static ClipPayload? DecodePayload(byte[] bytes)
    {
        try
        {
            using var reader = new BinaryReader(new MemoryStream(bytes));
            if (!reader.ReadBytes(4).AsSpan().SequenceEqual(PayloadMagic) || reader.ReadByte() != PayloadVersion) return null;
            var count = reader.ReadInt32();
            var representations = new List<Representation>(Math.Clamp(count, 0, 64));
            for (var index = 0; index < count; index++)
            {
                var name = Encoding.UTF8.GetString(ReadBlock(reader));
                representations.Add(new Representation(name, ReadBlock(reader)));
            }
            return new ClipPayload(representations);
        }
        catch (EndOfStreamException)
        {
            return null;
        }
    }

    private static byte[] ReadBlock(BinaryReader reader)
    {
        var length = reader.ReadInt32();
        if (length < 0 || length > reader.BaseStream.Length - reader.BaseStream.Position) throw new EndOfStreamException();
        return reader.ReadBytes(length);
    }

    private static void WriteAtomic(string path, byte[] bytes)
    {
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        var temporary = $"{path}.{Guid.NewGuid():N}.tmp";
        File.WriteAllBytes(temporary, bytes);
        File.Move(temporary, path, overwrite: true);
    }

    private static void TryDelete(string path)
    {
        try
        {
            File.Delete(path);
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
        }
    }
}

[JsonSourceGenerationOptions(
    PropertyNamingPolicy = JsonKnownNamingPolicy.CamelCase,
    DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull,
    UseStringEnumConverter = true,
    WriteIndented = true)]
[JsonSerializable(typeof(LibraryIndex))]
[JsonSerializable(typeof(PreferencesData))]
internal sealed partial class InsertJson : JsonSerializerContext;
