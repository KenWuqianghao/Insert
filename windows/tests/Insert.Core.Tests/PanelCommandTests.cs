namespace Insert.Core.Tests;

public sealed class PanelCommandTests
{
    private static readonly PanelMode Browsing = new PanelMode.Browsing();
    private static readonly PanelMode Previewing = new PanelMode.Previewing();
    private static readonly PanelMode Naming = new PanelMode.Naming(new NameTarget.NewPinboard(new HashSet<Guid>()));

    private static PanelCommand? Command(TrayKey key, ModifierKeys modifiers = ModifierKeys.None, char character = '\0', PanelMode? mode = null, bool queryIsEmpty = true) =>
        PanelCommand.From(new KeyPress(key, modifiers, character), mode ?? Browsing, queryIsEmpty);

    private static PanelCommand? Ctrl(char character, PanelMode? mode = null) =>
        Command(TrayKey.Character, ModifierKeys.Control, character, mode);

    [Fact]
    public void CtrlDigitsActivateByIndex()
    {
        for (var digit = '1'; digit <= '9'; digit++)
            Assert.Equal(new PanelCommand.ActivateIndex(digit - '1'), Ctrl(digit));
        Assert.Null(Ctrl('0'));
        Assert.Null(Command(TrayKey.Character, character: '1'));
    }

    [Fact]
    public void CtrlShortcuts()
    {
        Assert.Equal(new PanelCommand.Activate(PasteAction.Copy), Ctrl('c'));
        Assert.Equal(new PanelCommand.Activate(PasteAction.Copy), Ctrl('C'));
        Assert.Equal(new PanelCommand.SelectAll(), Ctrl('a'));
        Assert.Equal(new PanelCommand.OpenSettings(), Ctrl(','));
        Assert.Equal(new PanelCommand.SwitchScope(-1), Ctrl('['));
        Assert.Equal(new PanelCommand.SwitchScope(1), Ctrl(']'));
        Assert.Equal(new PanelCommand.DeleteSelection(), Command(TrayKey.Backspace, ModifierKeys.Control, queryIsEmpty: false));
        Assert.Equal(new PanelCommand.DeleteSelection(), Command(TrayKey.Delete, ModifierKeys.Control, queryIsEmpty: false));
        Assert.Null(Ctrl('x'));
        Assert.Null(Command(TrayKey.Character, ModifierKeys.Control | ModifierKeys.Shift, 'c'));
    }

    [Fact]
    public void CtrlQAndCtrlWHideInEveryMode()
    {
        foreach (var mode in new[] { Browsing, Previewing, Naming })
        {
            Assert.Equal(new PanelCommand.Hide(), Ctrl('q', mode));
            Assert.Equal(new PanelCommand.Hide(), Ctrl('W', mode));
        }
    }

    [Fact]
    public void ArrowsMoveAndShiftArrowsExtend()
    {
        Assert.Equal(new PanelCommand.Move(-1), Command(TrayKey.Left));
        Assert.Equal(new PanelCommand.Move(-1), Command(TrayKey.Up));
        Assert.Equal(new PanelCommand.Move(1), Command(TrayKey.Right));
        Assert.Equal(new PanelCommand.Move(1), Command(TrayKey.Down));
        Assert.Equal(new PanelCommand.Extend(-1), Command(TrayKey.Left, ModifierKeys.Shift));
        Assert.Equal(new PanelCommand.Extend(-1), Command(TrayKey.Up, ModifierKeys.Shift));
        Assert.Equal(new PanelCommand.Extend(1), Command(TrayKey.Right, ModifierKeys.Shift));
        Assert.Equal(new PanelCommand.Extend(1), Command(TrayKey.Down, ModifierKeys.Shift));
        Assert.Null(Command(TrayKey.Left, ModifierKeys.Alt));
    }

    [Fact]
    public void EnterPastesAndShiftEnterPastesPlainText()
    {
        Assert.Equal(new PanelCommand.Activate(PasteAction.Paste), Command(TrayKey.Enter, queryIsEmpty: false));
        Assert.Equal(new PanelCommand.Activate(PasteAction.PastePlainText), Command(TrayKey.Enter, ModifierKeys.Shift));
    }

    [Fact]
    public void TabAndShiftTabSwitchScope()
    {
        Assert.Equal(new PanelCommand.SwitchScope(1), Command(TrayKey.Tab));
        Assert.Equal(new PanelCommand.SwitchScope(-1), Command(TrayKey.Tab, ModifierKeys.Shift));
    }

    [Fact]
    public void EscapeCancelsWithoutModifiers()
    {
        Assert.Equal(new PanelCommand.Cancel(), Command(TrayKey.Escape));
        Assert.Null(Command(TrayKey.Escape, ModifierKeys.Shift));
    }

    [Fact]
    public void SpaceTogglesPreviewOnlyWhenQueryIsEmptyOrPreviewing()
    {
        Assert.Equal(new PanelCommand.TogglePreview(), Command(TrayKey.Space));
        Assert.Equal(new PanelCommand.TogglePreview(), Command(TrayKey.Space, mode: Previewing, queryIsEmpty: false));
        Assert.Null(Command(TrayKey.Space, queryIsEmpty: false));
        Assert.Null(Command(TrayKey.Space, ModifierKeys.Shift));
    }

    [Fact]
    public void BackspaceDeletesOnlyWhenQueryIsEmpty()
    {
        Assert.Equal(new PanelCommand.DeleteSelection(), Command(TrayKey.Backspace));
        Assert.Equal(new PanelCommand.DeleteSelection(), Command(TrayKey.Delete));
        Assert.Null(Command(TrayKey.Backspace, queryIsEmpty: false));
        Assert.Null(Command(TrayKey.Backspace, ModifierKeys.Shift));
    }

    [Fact]
    public void NamingModeTakesOnlyEscape()
    {
        Assert.Equal(new PanelCommand.Cancel(), Command(TrayKey.Escape, mode: Naming));
        Assert.Null(Command(TrayKey.Enter, mode: Naming));
        Assert.Null(Command(TrayKey.Down, mode: Naming));
        Assert.Null(Command(TrayKey.Backspace, mode: Naming));
        Assert.Null(Ctrl('a', Naming));
        Assert.Null(Ctrl('1', Naming));
    }
}
