namespace Insert.Core.Tests;

public sealed class PanelModelTests : IDisposable
{
    private readonly TempDirectory _dir = new();
    private readonly ClipLibrary _library;

    public PanelModelTests() => _library = new ClipLibrary(new ClipStorage(_dir.Path), 100);

    public void Dispose() => _dir.Dispose();

    private PanelModel Model() => new(_library, after: (_, action) => action());

    /// <summary>Inserts the texts in order, so the last text is the first card.</summary>
    private Guid[] Insert(params string[] texts) =>
        texts.Select(text =>
        {
            _library.Insert(Fixtures.Capture(text));
            return _library.Clips[0].Id;
        }).ToArray();

    private static Guid[] Visible(PanelModel model) => model.VisibleClips.Select(clip => clip.Id).ToArray();

    [Fact]
    public void SearchResetsTheSelectionToTheFirstMatch()
    {
        var ids = Insert("apple", "banana", "apricot");
        var model = Model();
        model.Click(ids[1], shift: false, control: false);

        model.Query = "AP";

        Assert.Equal([ids[2], ids[0]], Visible(model));
        Assert.Equal(ids[2], model.Selection.Primary);
        Assert.Equal([ids[2]], model.Selection.Ids);
    }

    [Fact]
    public void EmptyStates()
    {
        var model = Model();
        Assert.Equal(EmptyState.NoHistory, model.EmptyState);

        Insert("text");
        Assert.Null(model.EmptyState);

        model.Query = "missing";
        Assert.Equal(EmptyState.NoMatches, model.EmptyState);
        model.Query = "";
        model.KindFilter = ClipKind.Image;
        Assert.Equal(EmptyState.NoMatches, model.EmptyState);
        model.KindFilter = null;

        model.Scope = new Scope(_library.AddPinboard("Empty"));
        Assert.Equal(EmptyState.EmptyPinboard, model.EmptyState);
    }

    [Fact]
    public void TabCyclesThroughHistoryAndPinboards()
    {
        var first = _library.AddPinboard("One");
        var second = _library.AddPinboard("Two");
        var model = Model();
        var tab = new PanelCommand.SwitchScope(1);

        model.Perform(tab);
        Assert.Equal(new Scope(first), model.Scope);
        model.Perform(tab);
        Assert.Equal(new Scope(second), model.Scope);
        model.Perform(tab);
        Assert.Equal(Scope.History, model.Scope);
        model.Perform(new PanelCommand.SwitchScope(-1));
        Assert.Equal(new Scope(second), model.Scope);
    }

    [Fact]
    public void DeletingThePrimarySelectsTheNextCard()
    {
        var ids = Insert("a", "b", "c");
        var model = Model();
        model.Click(ids[1], shift: false, control: false);

        model.Perform(new PanelCommand.DeleteSelection());

        Assert.Equal([ids[2], ids[0]], Visible(model));
        Assert.Equal(ids[0], model.Selection.Primary);
        Assert.Empty(model.DeletingIds);
        Assert.DoesNotContain(_library.Clips, clip => clip.Id == ids[1]);
    }

    [Fact]
    public void CancelClosesPreviewThenClearsQueryThenHides()
    {
        Insert("apple", "banana");
        var model = Model();
        var hides = 0;
        model.Hide = () => hides++;
        model.Query = "apple";
        model.Perform(new PanelCommand.TogglePreview());
        Assert.IsType<PanelMode.Previewing>(model.Mode);

        model.Perform(new PanelCommand.Cancel());
        Assert.IsType<PanelMode.Browsing>(model.Mode);
        Assert.Equal("apple", model.Query);

        model.Perform(new PanelCommand.Cancel());
        Assert.Equal("", model.Query);
        Assert.Equal(2, model.VisibleClips.Count);
        Assert.Equal(0, hides);

        model.Perform(new PanelCommand.Cancel());
        Assert.Equal(1, hides);
    }

    [Fact]
    public void CommitNameForANewPinboardWithoutClipsShowsIt()
    {
        Insert("a");
        var model = Model();
        model.BeginNaming(new NameTarget.NewPinboard(new HashSet<Guid>()));

        model.CommitName("  Work  ");

        var board = Assert.Single(_library.Pinboards);
        Assert.Equal("Work", board.Name);
        Assert.Equal(new Scope(board.Id), model.Scope);
        Assert.IsType<PanelMode.Browsing>(model.Mode);
        Assert.Empty(model.VisibleClips);
    }

    [Fact]
    public void CommitNameForANewPinboardWithClipsPinsThem()
    {
        var ids = Insert("a", "b");
        var model = Model();
        model.BeginNaming(new NameTarget.NewPinboard(new HashSet<Guid> { ids[0] }));

        model.CommitName("Work");

        var board = Assert.Single(_library.Pinboards);
        Assert.Equal(Scope.History, model.Scope);
        Assert.Equal(board.Id, _library.Clips.Single(clip => clip.Id == ids[0]).PinboardId);
        Assert.Null(_library.Clips.Single(clip => clip.Id == ids[1]).PinboardId);
    }
}
