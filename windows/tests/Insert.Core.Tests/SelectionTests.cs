namespace Insert.Core.Tests;

public sealed class SelectionTests
{
    private static Guid[] Ids(int count) => Enumerable.Range(0, count).Select(_ => Guid.NewGuid()).ToArray();

    private static void AssertSelection(Selection selection, IEnumerable<Guid> ids, Guid? primary, Guid? anchor)
    {
        Assert.True(selection.Ids.SetEquals(ids), "selected ids differ");
        Assert.Equal(primary, selection.Primary);
        Assert.Equal(anchor, selection.Anchor);
    }

    private static void AssertOnly(Selection selection, Guid id) => AssertSelection(selection, [id], id, id);

    private static void AssertEmpty(Selection selection) => AssertSelection(selection, [], null, null);

    [Fact]
    public void MoveClampsAndStartsAtIndexZero()
    {
        var v = Ids(3);
        var selection = new Selection();
        selection.MoveBy(1, v);
        AssertOnly(selection, v[1]);
        selection.MoveBy(5, v);
        Assert.Equal(v[2], selection.Primary);
        selection.MoveBy(-9, v);
        Assert.Equal(v[0], selection.Primary);
        selection.MoveBy(1, []);
        AssertEmpty(selection);
    }

    [Fact]
    public void ExtendSelectsFromAnchorToTarget()
    {
        var v = Ids(4);
        var selection = Selection.Only(v[1]);
        selection.ExtendBy(1, v);
        selection.ExtendBy(1, v);
        AssertSelection(selection, v[1..4], v[3], v[1]);
        selection.ExtendBy(-3, v);
        Assert.True(selection.Ids.SetEquals(v[0..2]));
    }

    [Fact]
    public void SelectRangeWithoutAnchorSelectsOnlyTheTarget()
    {
        var v = Ids(3);
        var selection = new Selection();
        selection.SelectRange(v[2], v);
        AssertOnly(selection, v[2]);
        var hiddenAnchor = Selection.Only(Guid.NewGuid());
        hiddenAnchor.SelectRange(v[1], v);
        AssertOnly(hiddenAnchor, v[1]);
    }

    [Fact]
    public void ToggleAddsAndRemovesAndMovesThePrimary()
    {
        var v = Ids(3);
        var selection = Selection.Only(v[0]);
        selection.Toggle(v[2], v);
        AssertSelection(selection, [v[0], v[2]], v[2], v[0]);
        selection.Toggle(v[2], v);
        AssertSelection(selection, [v[0]], v[0], v[0]);
        selection.Toggle(v[0], v);
        AssertEmpty(selection);
    }

    [Fact]
    public void SelectAllKeepsAVisiblePrimary()
    {
        var v = Ids(3);
        var selection = Selection.Only(v[1]);
        selection.SelectAll(v);
        AssertSelection(selection, v, v[1], v[0]);
    }

    [Fact]
    public void ReconcileKeepsSurvivorsOrSelectsTheFirst()
    {
        var v = Ids(4);
        var selection = new Selection();
        selection.SelectRange(v[1], v);
        selection.SelectRange(v[3], v);
        selection.Reconcile([v[0], v[1], v[2]]);
        AssertSelection(selection, v[1..3], v[1], v[1]);
        selection.Reconcile([v[0], v[3]]);
        AssertOnly(selection, v[0]);
    }

    [Fact]
    public void AfterDeletingThePrimarySelectsTheNextCard()
    {
        var v = Ids(4);
        var selection = Selection.Only(v[1]);
        selection.ExtendBy(1, v);
        AssertOnly(selection.AfterDeleting(v[1..3].ToHashSet(), v), v[3]);
        AssertOnly(Selection.Only(v[3]).AfterDeleting(new HashSet<Guid> { v[3] }, v), v[2]);
        AssertEmpty(Selection.Only(v[0]).AfterDeleting(v.ToHashSet(), v));
    }

    [Fact]
    public void AfterDeletingOthersKeepsThePrimary()
    {
        var v = Ids(3);
        var selection = Selection.Only(v[0]);
        selection.SelectRange(v[2], v);
        var next = selection.AfterDeleting(new HashSet<Guid> { v[0] }, v);
        AssertSelection(next, v[1..3], v[2], v[2]);
    }
}
