namespace Insert.Core;

/// <summary>The clips and pinboards. Call it from one thread only.</summary>
public sealed class ClipLibrary
{
    public static readonly IReadOnlyList<int> HistoryLimitOptions = [100, 500, 1_000, 5_000];
    public const int DefaultHistoryLimit = 500;

    private readonly ClipStorage _storage;
    private readonly Func<DateTimeOffset> _now;
    private List<Clip> _clips;
    private List<Pinboard> _pinboards;
    private int _historyLimit;
    /// <summary>Clips that left the library. Their files stay until the index without them is saved.</summary>
    private readonly HashSet<Guid> _removedFiles = [];

    public ClipLibrary(ClipStorage storage, int historyLimit, Func<DateTimeOffset>? now = null)
    {
        _storage = storage;
        _now = now ?? (() => DateTimeOffset.Now);
        _historyLimit = historyLimit;

        var index = storage.LoadIndex();
        _clips = index.Clips.Where(clip => storage.HasPayload(clip.Id)).ToList();
        _pinboards = index.Pinboards.ToList();
        if (!storage.HasCorruptBackup) storage.RemoveOrphans(_clips.Select(clip => clip.Id).ToHashSet());
    }

    /// <summary>The library changed. Call <see cref="Flush"/> soon after, to save the index.</summary>
    public event Action? Changed;

    /// <summary>Newest first.</summary>
    public IReadOnlyList<Clip> Clips => _clips;

    public IReadOnlyList<Pinboard> Pinboards => _pinboards;

    public bool HasUnsavedChanges { get; private set; }

    public int HistoryLimit
    {
        get => _historyLimit;
        set
        {
            _historyLimit = value;
            Update(clips => Trim(clips, value));
        }
    }

    public void Insert(CapturedClip capture)
    {
        if (_clips.FirstOrDefault(clip => clip.ContentHash == capture.Clip.ContentHash) is { } existing)
        {
            Update(clips => MoveToFront(existing.Id, clips, clip => clip with { Source = capture.Clip.Source ?? clip.Source }));
            return;
        }

        _storage.Write(capture);
        Update(clips =>
        {
            clips.Insert(0, capture.Clip);
            Trim(clips, _historyLimit);
        });
    }

    public void MoveToFront(Guid id) => Update(clips => MoveToFront(id, clips, clip => clip));

    public void Delete(IReadOnlySet<Guid> ids) => Update(clips => clips.RemoveAll(clip => ids.Contains(clip.Id)));

    /// <summary>Deletes all clips that are not in a pinboard.</summary>
    public void ClearHistory() => Update(clips => clips.RemoveAll(clip => clip.PinboardId is null));

    public void SetPinboard(Guid? pinboardId, IReadOnlySet<Guid> ids) => Update(clips =>
    {
        for (var index = 0; index < clips.Count; index++)
            if (ids.Contains(clips[index].Id)) clips[index] = clips[index] with { PinboardId = pinboardId };
        Trim(clips, _historyLimit);
    });

    public void Rename(Guid id, string? title) => Update(clips =>
    {
        var index = clips.FindIndex(clip => clip.Id == id);
        if (index >= 0) clips[index] = clips[index] with { CustomTitle = title };
    });

    public ClipPayload? Payload(Guid id) => _storage.LoadPayload(id);

    public string ThumbnailPath(Guid id) => _storage.ThumbnailPath(id);

    public Guid AddPinboard(string name)
    {
        var used = _pinboards.Select(pinboard => pinboard.Color).ToHashSet();
        var palette = Enum.GetValues<PinboardColor>();
        var color = palette.Where(candidate => !used.Contains(candidate)).Cast<PinboardColor?>().FirstOrDefault()
            ?? palette[_pinboards.Count % palette.Length];
        var pinboard = new Pinboard(Guid.NewGuid(), name, color);
        _pinboards.Add(pinboard);
        MarkChanged();
        return pinboard.Id;
    }

    public void RenamePinboard(Guid id, string name) => UpdatePinboard(id, pinboard => pinboard with { Name = name });

    public void SetPinboardColor(Guid id, PinboardColor color) => UpdatePinboard(id, pinboard => pinboard with { Color = color });

    /// <summary>The clips of the pinboard go back to the history. The history limit then applies to them.</summary>
    public void DeletePinboard(Guid id)
    {
        if (_pinboards.RemoveAll(pinboard => pinboard.Id == id) == 0) return;
        var clips = _clips.Select(clip => clip.PinboardId == id ? clip with { PinboardId = null } : clip).ToList();
        Trim(clips, _historyLimit);
        Commit(clips);
        MarkChanged();
    }

    public void Flush()
    {
        if (!HasUnsavedChanges) return;
        _storage.SaveIndex(new LibraryIndex(_clips.ToList(), _pinboards.ToList()));
        HasUnsavedChanges = false;
        foreach (var id in _removedFiles) _storage.RemoveFiles(id);
        _removedFiles.Clear();
    }

    private void Update(Action<List<Clip>> change)
    {
        var next = _clips.ToList();
        change(next);
        if (next.SequenceEqual(_clips)) return;
        Commit(next);
        MarkChanged();
    }

    private void Commit(List<Clip> next)
    {
        var kept = next.Select(clip => clip.Id).ToHashSet();
        _removedFiles.UnionWith(_clips.Where(clip => !kept.Contains(clip.Id)).Select(clip => clip.Id));
        _clips = next;
    }

    private void UpdatePinboard(Guid id, Func<Pinboard, Pinboard> change)
    {
        var index = _pinboards.FindIndex(pinboard => pinboard.Id == id);
        if (index < 0) return;
        _pinboards[index] = change(_pinboards[index]);
        MarkChanged();
    }

    private void MarkChanged()
    {
        HasUnsavedChanges = true;
        Changed?.Invoke();
    }

    private void MoveToFront(Guid id, List<Clip> clips, Func<Clip, Clip> adjust)
    {
        var index = clips.FindIndex(clip => clip.Id == id);
        if (index < 0) return;
        var clip = clips[index];
        clips.RemoveAt(index);
        clips.Insert(0, adjust(clip with { CreatedAt = _now() }));
    }

    private static void Trim(List<Clip> clips, int limit)
    {
        var unpinned = 0;
        clips.RemoveAll(clip => clip.PinboardId is null && ++unpinned > limit);
    }
}
