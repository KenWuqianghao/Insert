namespace Insert.Core.Tests;

public sealed class HoldTriggerTests
{
    private sealed class FakeHost : IHoldTriggerHost
    {
        public int TimersStarted;
        public int TimersCancelled;
        public int Opens;
        public int Releases;
        public bool OpenResult = true;

        public Point2 Pointer { get; set; } = new(100, 100);
        public bool AnyMouseButtonDown { get; set; }
        public bool OwnWindowIsActive { get; set; }
        public bool OtherKeyIsDown { get; set; }
        public bool MenuIsOpen { get; set; }
        public ModifierKeys CurrentModifiers { get; set; }

        public void StartTimer(TimeSpan delay)
        {
            Assert.Equal(HoldTrigger.Delay, delay);
            TimersStarted++;
        }

        public void CancelTimer() => TimersCancelled++;

        public bool Open()
        {
            Opens++;
            return OpenResult;
        }

        public void Release() => Releases++;
    }

    private readonly FakeHost _host = new();
    private readonly HoldTrigger _trigger;

    public HoldTriggerTests() => _trigger = new HoldTrigger(_host);

    private void Press(ModifierKeys held)
    {
        _host.CurrentModifiers = held;
        _trigger.ModifiersChanged(held);
    }

    [Fact]
    public void LonePressStartsTheTimerAndTheTimerOpensTheRing()
    {
        Press(ModifierKeys.Alt);
        Assert.Equal(HoldState.Waiting, _trigger.State);
        Assert.Equal(1, _host.TimersStarted);

        _host.Pointer = new Point2(106, 108);
        _trigger.TimerFired();

        Assert.Equal(HoldState.Holding, _trigger.State);
        Assert.Equal(1, _host.Opens);
    }

    [Fact]
    public void PressWhileAnotherModifierIsHeldDoesNotStart()
    {
        Press(ModifierKeys.Control);
        Press(ModifierKeys.Control | ModifierKeys.Alt);
        Press(ModifierKeys.Alt);

        Assert.Equal(HoldState.Idle, _trigger.State);
        Assert.Equal(0, _host.TimersStarted);
    }

    [Fact]
    public void PointerMovedTooFarCancelsAtTimerTime()
    {
        Press(ModifierKeys.Alt);
        _host.Pointer = new Point2(111, 100);
        _trigger.TimerFired();

        Assert.Equal(HoldState.Idle, _trigger.State);
        Assert.Equal(0, _host.Opens);
    }

    [Fact]
    public void InterruptionDuringTheWaitCancels()
    {
        Press(ModifierKeys.Alt);
        _trigger.Interrupted();

        Assert.Equal(HoldState.Idle, _trigger.State);
        Assert.Equal(1, _host.TimersCancelled);
        _trigger.TimerFired();
        Assert.Equal(0, _host.Opens);
    }

    [Fact]
    public void SecondModifierDuringTheWaitCancels()
    {
        Press(ModifierKeys.Alt);
        Press(ModifierKeys.Alt | ModifierKeys.Shift);

        Assert.Equal(HoldState.Idle, _trigger.State);
        Assert.Equal(1, _host.TimersCancelled);
    }

    [Fact]
    public void ReleaseWhileHoldingCallsReleaseOnce()
    {
        Press(ModifierKeys.Alt);
        _trigger.TimerFired();
        Press(ModifierKeys.Alt | ModifierKeys.Shift);
        Assert.Equal(HoldState.Holding, _trigger.State);

        Press(ModifierKeys.None);
        Press(ModifierKeys.None);

        Assert.Equal(HoldState.Idle, _trigger.State);
        Assert.Equal(1, _host.Releases);
    }

    [Fact]
    public void MouseButtonOrOtherKeyDownOrOwnWindowActiveBlocksTheStart()
    {
        _host.AnyMouseButtonDown = true;
        Press(ModifierKeys.Alt);
        Press(ModifierKeys.None);
        _host.AnyMouseButtonDown = false;
        _host.OtherKeyIsDown = true;
        Press(ModifierKeys.Alt);
        Press(ModifierKeys.None);
        _host.OtherKeyIsDown = false;
        _host.OwnWindowIsActive = true;
        Press(ModifierKeys.Alt);

        Assert.Equal(HoldState.Idle, _trigger.State);
        Assert.Equal(0, _host.TimersStarted);
    }

    [Fact]
    public void OpenMenuBlocksTheOpen()
    {
        Press(ModifierKeys.Alt);
        _host.MenuIsOpen = true;
        _trigger.TimerFired();

        Assert.Equal(HoldState.Idle, _trigger.State);
        Assert.Equal(0, _host.Opens);
    }

    [Fact]
    public void FailedOpenReturnsToIdle()
    {
        _host.OpenResult = false;
        Press(ModifierKeys.Alt);
        _trigger.TimerFired();

        Assert.Equal(HoldState.Idle, _trigger.State);
        Assert.Equal(1, _host.Opens);
        Press(ModifierKeys.None);
        Assert.Equal(0, _host.Releases);
    }

    [Fact]
    public void WindowsModifierOpensTheRing()
    {
        _trigger.Modifier = RingModifier.Windows;
        Press(ModifierKeys.Alt);
        Assert.Equal(HoldState.Idle, _trigger.State);
        Press(ModifierKeys.None);

        Press(ModifierKeys.Windows);
        _trigger.TimerFired();

        Assert.Equal(HoldState.Holding, _trigger.State);
        Press(ModifierKeys.None);
        Assert.Equal(1, _host.Releases);
    }
}
