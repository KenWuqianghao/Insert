using System.Collections.ObjectModel;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using CommunityToolkit.Mvvm.ComponentModel;
using Insert.Core;
using Insert.Services;

namespace Insert.Views;

public sealed partial class ScopeTabViewModel(Scope scope, string title, IBrush? dot) : ObservableObject
{
    public Scope Scope { get; } = scope;
    public string Title { get; } = title;
    public IBrush? Dot { get; } = dot;
    public Pinboard? Pinboard { get; init; }

    [ObservableProperty]
    public partial bool IsSelected { get; set; }
}

/// <summary>The state of the tray view. It reads the core <see cref="PanelModel"/> after each change.</summary>
public sealed partial class TrayViewModel : ObservableObject
{
    private readonly Dictionary<Guid, CardViewModel> _cards = [];
    private readonly Dictionary<Guid, Bitmap?> _thumbnails = [];
    private readonly IconStore _icons;
    private readonly Preferences _preferences;

    public TrayViewModel(PanelModel model, IconStore icons, Preferences preferences)
    {
        Model = model;
        _icons = icons;
        _preferences = preferences;
        model.Changed += Sync;
        preferences.Changed += _ => OnPropertyChanged(nameof(CapturePaused));
        Sync();
    }

    public PanelModel Model { get; }
    public IconStore Icons => _icons;
    public ObservableCollection<CardViewModel> Cards { get; } = [];
    public ObservableCollection<ScopeTabViewModel> Scopes { get; } = [];

    /// <summary>The primary card changed. The view scrolls to it.</summary>
    public event Action<int>? PrimaryMoved;

    public string Query
    {
        get => Model.Query;
        set
        {
            if (Model.Query == value) return;
            Model.Query = value;
            OnPropertyChanged();
        }
    }

    public bool HasQuery => Model.Query.Length > 0;
    public bool CapturePaused => _preferences.Data.CapturePaused;
    public bool IsPreviewing => Model.Mode is PanelMode.Previewing;
    public bool IsNaming => Model.Mode is PanelMode.Naming;
    public string NamingTitle => Model.Mode is PanelMode.Naming naming ? naming.Target.PromptTitle : "";
    public string? KindFilterName => Model.KindFilter?.Name();
    public bool HasKindFilter => Model.KindFilter is not null;
    public bool HasCards => Model.EmptyState is null;

    [ObservableProperty]
    public partial PreviewViewModel? Preview { get; set; }

    public Geometry? EmptyIcon => Palette.Icon(Model.EmptyState switch
    {
        EmptyState.NoMatches => "DocumentSearch",
        EmptyState.EmptyPinboard => "Pin",
        _ => "Clipboard",
    });

    public string EmptyTitle => Model.EmptyState switch
    {
        EmptyState.NoMatches => "No Matches",
        EmptyState.EmptyPinboard => "Empty Pinboard",
        _ => "No Clips",
    };

    public string EmptyDetail => Model.EmptyState switch
    {
        EmptyState.NoMatches => "Change the search text or the type filter.",
        EmptyState.EmptyPinboard => "Right-click a clip and select Pin to.",
        _ => "Copy something. It shows here.",
    };

    public void RefreshTimes()
    {
        var now = DateTimeOffset.Now;
        foreach (var card in Cards) card.RefreshTime(now);
    }

    private Guid? _lastPrimary;

    private void Sync()
    {
        var library = Model.Library;
        var visible = Model.VisibleClips;

        foreach (var id in _cards.Keys.Where(id => library.Clips.All(clip => clip.Id != id)).ToList())
        {
            _cards.Remove(id);
            _thumbnails.Remove(id);
        }

        var target = visible.Select(clip =>
        {
            if (!_cards.TryGetValue(clip.Id, out var card))
            {
                card = new CardViewModel(clip, _icons, Thumbnail);
                _cards[clip.Id] = card;
            }
            card.Update(clip, library.Pinboards.FirstOrDefault(pinboard => pinboard.Id == clip.PinboardId));
            return card;
        }).ToList();
        Apply(Cards, target);

        for (var index = 0; index < target.Count; index++)
        {
            var card = target[index];
            card.IsSelected = Model.Selection.Contains(card.Clip.Id);
            card.IsPrimary = Model.Selection.Primary == card.Clip.Id;
            card.IsDeleting = Model.DeletingIds.Contains(card.Clip.Id);
            card.Badge = Model.IsControlHeld && index < 9 ? $"Ctrl+{index + 1}" : null;
        }

        SyncScopes();

        var primary = Model.PrimaryClip;
        Preview = IsPreviewing && primary is not null
            ? Preview?.Clip.Id == primary.Id ? Preview : new PreviewViewModel(primary, Model.Library, _icons, Thumbnail(primary.Id))
            : null;

        OnPropertyChanged(nameof(Query));
        OnPropertyChanged(nameof(HasQuery));
        OnPropertyChanged(nameof(IsPreviewing));
        OnPropertyChanged(nameof(IsNaming));
        OnPropertyChanged(nameof(NamingTitle));
        OnPropertyChanged(nameof(KindFilterName));
        OnPropertyChanged(nameof(HasKindFilter));
        OnPropertyChanged(nameof(HasCards));
        OnPropertyChanged(nameof(EmptyIcon));
        OnPropertyChanged(nameof(EmptyTitle));
        OnPropertyChanged(nameof(EmptyDetail));

        if (Model.Selection.Primary != _lastPrimary)
        {
            _lastPrimary = Model.Selection.Primary;
            var index = target.FindIndex(card => card.Clip.Id == _lastPrimary);
            if (index >= 0) PrimaryMoved?.Invoke(index);
        }
    }

    private void SyncScopes()
    {
        var library = Model.Library;
        var wanted = new List<(Scope Scope, string Title, Pinboard? Pinboard)> { (Scope.History, "Clipboard", null) };
        wanted.AddRange(library.Pinboards.Select(pinboard => (new Scope(pinboard.Id), pinboard.Name, (Pinboard?)pinboard)));

        var same = Scopes.Count == wanted.Count && Scopes.Zip(wanted).All(pair => pair.First.Scope == pair.Second.Scope
            && pair.First.Title == pair.Second.Title && pair.First.Pinboard?.Color == pair.Second.Pinboard?.Color);
        if (!same)
        {
            Scopes.Clear();
            foreach (var (scope, title, pinboard) in wanted)
                Scopes.Add(new ScopeTabViewModel(scope, title, pinboard is null ? null : Palette.PinboardBrush(pinboard.Color)) { Pinboard = pinboard });
        }
        foreach (var tab in Scopes) tab.IsSelected = tab.Scope == Model.Scope;
    }

    public Bitmap? Thumbnail(Guid id)
    {
        if (_thumbnails.TryGetValue(id, out var bitmap)) return bitmap;
        var path = Model.Library.ThumbnailPath(id);
        try
        {
            bitmap = File.Exists(path) ? new Bitmap(path) : null;
        }
        catch (Exception error) when (error is IOException or ArgumentException or NotSupportedException)
        {
            bitmap = null;
        }
        _thumbnails[id] = bitmap;
        return bitmap;
    }

    /// <summary>Changes <paramref name="items"/> into <paramref name="target"/> with few moves, so that the cards keep their views.</summary>
    private static void Apply<T>(ObservableCollection<T> items, IReadOnlyList<T> target) where T : class
    {
        var wanted = target.ToHashSet();
        for (var index = items.Count - 1; index >= 0; index--)
        {
            if (!wanted.Contains(items[index])) items.RemoveAt(index);
        }
        for (var index = 0; index < target.Count; index++)
        {
            if (index < items.Count && ReferenceEquals(items[index], target[index])) continue;
            var current = items.IndexOf(target[index]);
            if (current >= 0)
                items.Move(current, index);
            else
                items.Insert(index, target[index]);
        }
    }
}
