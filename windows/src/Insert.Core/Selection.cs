namespace Insert.Core;

/// <summary>The selected cards. Every method takes the visible clip ids in display order.</summary>
public sealed class Selection
{
    private readonly HashSet<Guid> _ids = [];

    public Selection()
    {
    }

    public static Selection Only(Guid? id)
    {
        var selection = new Selection { Primary = id, Anchor = id };
        if (id is { } value) selection._ids.Add(value);
        return selection;
    }

    public IReadOnlySet<Guid> Ids => _ids;
    public Guid? Primary { get; private set; }
    public Guid? Anchor { get; private set; }

    public bool Contains(Guid id) => _ids.Contains(id);

    public Selection Clone()
    {
        var copy = new Selection { Primary = Primary, Anchor = Anchor };
        copy._ids.UnionWith(_ids);
        return copy;
    }

    public void MoveBy(int offset, IReadOnlyList<Guid> visible) => Replace(Only(Neighbor(offset, visible)));

    public void ExtendBy(int offset, IReadOnlyList<Guid> visible)
    {
        if (Neighbor(offset, visible) is { } target)
            SelectRange(target, visible);
        else
            Replace(new Selection());
    }

    public void Toggle(Guid id, IReadOnlyList<Guid> visible)
    {
        if (_ids.Remove(id))
        {
            if (Primary == id)
                Primary = visible.Cast<Guid?>().FirstOrDefault(candidate => _ids.Contains(candidate!.Value));
            Anchor = Primary;
        }
        else
        {
            _ids.Add(id);
            Primary = id;
            Anchor ??= id;
        }
    }

    public void SelectRange(Guid id, IReadOnlyList<Guid> visible)
    {
        var targetIndex = IndexOf(visible, id);
        var anchorId = Anchor ?? Primary;
        var anchorIndex = anchorId is { } value ? IndexOf(visible, value) : -1;
        if (targetIndex < 0 || anchorIndex < 0)
        {
            Replace(Only(id));
            return;
        }

        _ids.Clear();
        for (var index = Math.Min(anchorIndex, targetIndex); index <= Math.Max(anchorIndex, targetIndex); index++)
            _ids.Add(visible[index]);
        Primary = id;
        Anchor = anchorId;
    }

    public void SelectAll(IReadOnlyList<Guid> visible)
    {
        _ids.Clear();
        _ids.UnionWith(visible);
        Primary = Primary is { } primary && _ids.Contains(primary) ? primary : visible.Count > 0 ? visible[0] : null;
        Anchor = visible.Count > 0 ? visible[0] : null;
    }

    public void Reconcile(IReadOnlyList<Guid> visible)
    {
        var survivors = visible.Where(_ids.Contains).ToList();
        if (survivors.Count == 0)
        {
            Replace(Only(visible.Count > 0 ? visible[0] : null));
            return;
        }

        _ids.Clear();
        _ids.UnionWith(survivors);
        if (Primary is not { } primary || !_ids.Contains(primary)) Primary = survivors[0];
        if (Anchor is not { } anchor || !_ids.Contains(anchor)) Anchor = Primary;
    }

    /// <summary>The selection to show after <paramref name="removed"/> leaves the list.</summary>
    public Selection AfterDeleting(IReadOnlySet<Guid> removed, IReadOnlyList<Guid> visible)
    {
        if (Primary is not { } primary || !removed.Contains(primary))
        {
            var kept = Clone();
            kept._ids.ExceptWith(removed);
            if (kept.Anchor is { } anchor && removed.Contains(anchor)) kept.Anchor = kept.Primary;
            return kept;
        }

        var remaining = visible.Where(id => !removed.Contains(id)).ToList();
        if (remaining.Count == 0) return new Selection();

        var firstRemoved = Math.Max(0, visible.ToList().FindIndex(removed.Contains));
        return Only(remaining[Math.Min(firstRemoved, remaining.Count - 1)]);
    }

    private Guid? Neighbor(int offset, IReadOnlyList<Guid> visible)
    {
        if (visible.Count == 0) return null;
        var current = Primary is { } primary ? Math.Max(0, IndexOf(visible, primary)) : 0;
        return visible[Math.Clamp(current + offset, 0, visible.Count - 1)];
    }

    private void Replace(Selection other)
    {
        _ids.Clear();
        _ids.UnionWith(other._ids);
        Primary = other.Primary;
        Anchor = other.Anchor;
    }

    private static int IndexOf(IReadOnlyList<Guid> visible, Guid id)
    {
        for (var index = 0; index < visible.Count; index++)
            if (visible[index] == id) return index;
        return -1;
    }
}
