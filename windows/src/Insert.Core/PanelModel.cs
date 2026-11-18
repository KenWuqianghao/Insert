namespace Insert.Core;

/// <summary>The history, or one pinboard.</summary>
public readonly record struct Scope(Guid? PinboardId)
{
    public static readonly Scope History = new(null);
    public bool IsHistory => PinboardId is null;
}

public abstract record NameTarget
{
    public sealed record ClipName(Guid Id) : NameTarget;
    public sealed record PinboardName(Guid Id) : NameTarget;

    /// <summary>A new pinboard. When <see cref="Pinning"/> is empty, the tray shows the new pinboard.</summary>
    public sealed record NewPinboard(IReadOnlySet<Guid> Pinning) : NameTarget
    {
        public bool Equals(NewPinboard? other) => other is not null && Pinning.SetEquals(other.Pinning);
        public override int GetHashCode() => Pinning.Count;
    }

    public string PromptTitle => this switch
    {
        ClipName => "Rename Clip",
        PinboardName => "Rename Pinboard",
        _ => "New Pinboard",
    };
}

/// <summary>The preview always shows the primary clip of the selection, so it holds no clip id.</summary>
public abstract record PanelMode
{
    public sealed record Browsing : PanelMode;
    public sealed record Previewing : PanelMode;
    public sealed record Naming(NameTarget Target) : PanelMode;
}

public enum EmptyState
{
    NoHistory,
    NoMatches,
    EmptyPinboard,
}

/// <summary>The state of the tray. The view reads it after each <see cref="Changed"/> event.</summary>
public sealed class PanelModel
{
    public static readonly TimeSpan DeleteAnimation = TimeSpan.FromMilliseconds(160);

    private readonly Action<TimeSpan, Action> _after;
    private Scope _scope = Scope.History;
    private string _query = "";
    private ClipKind? _kindFilter;
    private bool _isControlHeld;

    /// <param name="after">Runs an action after a delay on the thread of the model.</param>
    public PanelModel(ClipLibrary library, Action<TimeSpan, Action> after)
    {
        Library = library;
        _after = after;
        library.Changed += OnLibraryChanged;
        Refresh(resettingSelection: true);
    }

    public event Action? Changed;

    public Action Hide { get; set; } = () => { };
    public Action<Clip, PasteAction> ActivateClip { get; set; } = (_, _) => { };
    public Action ShowSettings { get; set; } = () => { };

    public ClipLibrary Library { get; }
    public IReadOnlyList<Clip> VisibleClips { get; private set; } = [];
    public Selection Selection { get; private set; } = new();
    public PanelMode Mode { get; private set; } = new PanelMode.Browsing();
    public IReadOnlySet<Guid> DeletingIds { get; private set; } = new HashSet<Guid>();
    public int ShowCount { get; private set; }

    public Scope Scope
    {
        get => _scope;
        set
        {
            if (_scope == value) return;
            _scope = value;
            Refresh(resettingSelection: true);
        }
    }

    public string Query
    {
        get => _query;
        set
        {
            if (_query == value) return;
            _query = value;
            Refresh(resettingSelection: true);
        }
    }

    public ClipKind? KindFilter
    {
        get => _kindFilter;
        set
        {
            if (_kindFilter == value) return;
            _kindFilter = value;
            Refresh(resettingSelection: true);
        }
    }

    /// <summary>While the user holds Ctrl, the cards show the numbers 1 to 9.</summary>
    public bool IsControlHeld
    {
        get => _isControlHeld;
        set
        {
            if (_isControlHeld == value) return;
            _isControlHeld = value;
            Changed?.Invoke();
        }
    }

    public Clip? PrimaryClip => Selection.Primary is { } id ? VisibleClips.FirstOrDefault(clip => clip.Id == id) : null;

    public EmptyState? EmptyState
    {
        get
        {
            if (VisibleClips.Count > 0) return null;
            if (Query.Length > 0 || KindFilter is not null) return Core.EmptyState.NoMatches;
            return Scope.IsHistory ? Core.EmptyState.NoHistory : Core.EmptyState.EmptyPinboard;
        }
    }

    public void PrepareToShow()
    {
        Mode = new PanelMode.Browsing();
        _kindFilter = null;
        _query = "";
        ShowCount++;
        Refresh(resettingSelection: true);
    }

    public void DidHide()
    {
        Mode = new PanelMode.Browsing();
        _isControlHeld = false;
        Changed?.Invoke();
    }

    public void Perform(PanelCommand command)
    {
        switch (command)
        {
            case PanelCommand.Move move:
                Selection.MoveBy(move.Offset, VisibleIds);
                break;
            case PanelCommand.Extend extend:
                Selection.ExtendBy(extend.Offset, VisibleIds);
                break;
            case PanelCommand.SelectAll:
                Selection.SelectAll(VisibleIds);
                break;
            case PanelCommand.Activate activate:
                if (PrimaryClip is { } clip) ActivateClip(clip, activate.Action);
                return;
            case PanelCommand.ActivateIndex activateIndex:
                if (activateIndex.Index < VisibleClips.Count) ActivateClip(VisibleClips[activateIndex.Index], PasteAction.Paste);
                return;
            case PanelCommand.DeleteSelection:
                Delete(Selection.Ids);
                return;
            case PanelCommand.TogglePreview:
                Mode = Mode is PanelMode.Browsing && PrimaryClip is not null ? new PanelMode.Previewing() : new PanelMode.Browsing();
                break;
            case PanelCommand.Cancel:
                Cancel();
                return;
            case PanelCommand.SwitchScope switchScope:
                SwitchScope(switchScope.Offset);
                return;
            case PanelCommand.OpenSettings:
                ShowSettings();
                return;
            case PanelCommand.Hide:
                Hide();
                return;
        }
        Changed?.Invoke();
    }

    public void Click(Guid id, bool shift, bool control)
    {
        if (shift)
            Selection.SelectRange(id, VisibleIds);
        else if (control)
            Selection.Toggle(id, VisibleIds);
        else
            Selection = Selection.Only(id);
        Changed?.Invoke();
    }

    public void Preview(Clip clip)
    {
        Selection = Selection.Only(clip.Id);
        Mode = new PanelMode.Previewing();
        Changed?.Invoke();
    }

    /// <summary>A menu on a selected card acts on the whole selection. A menu on another card acts on that card only.</summary>
    public IReadOnlySet<Guid> Targets(Clip clip) =>
        Selection.Contains(clip.Id) ? Selection.Ids.ToHashSet() : new HashSet<Guid> { clip.Id };

    /// <summary>The cards fade out first. Then the library removes them.</summary>
    public void Delete(IReadOnlySet<Guid> ids)
    {
        if (DeletingIds.Count > 0 || ids.Count == 0) return;
        var removing = ids.ToHashSet();
        Selection = Selection.AfterDeleting(removing, VisibleIds);
        DeletingIds = removing;
        Changed?.Invoke();

        _after(DeleteAnimation, () =>
        {
            DeletingIds = new HashSet<Guid>();
            Library.Delete(removing);
            Changed?.Invoke();
        });
    }

    public void BeginNaming(NameTarget target)
    {
        Mode = new PanelMode.Naming(target);
        Changed?.Invoke();
    }

    public string CurrentName(NameTarget target) => target switch
    {
        NameTarget.ClipName clip => Library.Clips.FirstOrDefault(c => c.Id == clip.Id)?.CustomTitle ?? "",
        NameTarget.PinboardName pinboard => Library.Pinboards.FirstOrDefault(p => p.Id == pinboard.Id)?.Name ?? "",
        _ => "",
    };

    public void CommitName(string text)
    {
        if (Mode is not PanelMode.Naming naming) return;
        var name = text.Trim();
        Mode = new PanelMode.Browsing();

        switch (naming.Target)
        {
            case NameTarget.ClipName clip:
                Library.Rename(clip.Id, name.Length == 0 ? null : name);
                break;
            case NameTarget.PinboardName pinboard when name.Length > 0:
                Library.RenamePinboard(pinboard.Id, name);
                break;
            case NameTarget.NewPinboard newPinboard when name.Length > 0:
                var id = Library.AddPinboard(name);
                if (newPinboard.Pinning.Count == 0)
                    Scope = new Scope(id);
                else
                    Library.SetPinboard(id, newPinboard.Pinning);
                break;
        }
        Changed?.Invoke();
    }

    private IReadOnlyList<Guid> VisibleIds => VisibleClips.Select(clip => clip.Id).ToList();

    private void Cancel()
    {
        if (Mode is not PanelMode.Browsing)
        {
            Mode = new PanelMode.Browsing();
            Changed?.Invoke();
        }
        else if (Query.Length > 0)
        {
            Query = "";
        }
        else
        {
            Hide();
        }
    }

    private void SwitchScope(int offset)
    {
        var scopes = new List<Scope> { Scope.History };
        scopes.AddRange(Library.Pinboards.Select(pinboard => new Scope(pinboard.Id)));
        var current = Math.Max(0, scopes.IndexOf(Scope));
        Scope = scopes[((current + offset) % scopes.Count + scopes.Count) % scopes.Count];
    }

    private void OnLibraryChanged()
    {
        if (Scope.PinboardId is { } id && Library.Pinboards.All(pinboard => pinboard.Id != id))
            _scope = Scope.History;
        Refresh(resettingSelection: false);
    }

    private void Refresh(bool resettingSelection)
    {
        var query = Query.Trim();
        VisibleClips = Library.Clips.Where(clip =>
            (Scope.PinboardId is not { } pinboard || clip.PinboardId == pinboard)
            && (KindFilter is not { } kind || clip.Kind == kind)
            && (query.Length == 0 || clip.Matches(query))).ToList();

        var ids = VisibleIds;
        if (resettingSelection)
            Selection = Selection.Only(ids.Count > 0 ? ids[0] : null);
        else
            Selection.Reconcile(ids);

        if (Mode is PanelMode.Previewing && Selection.Primary is null) Mode = new PanelMode.Browsing();
        Changed?.Invoke();
    }
}
