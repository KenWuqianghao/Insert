namespace Insert.Core;

public readonly record struct Point2(double X, double Y)
{
    public double DistanceTo(Point2 other) => Math.Sqrt((X - other.X) * (X - other.X) + (Y - other.Y) * (Y - other.Y));
}

/// <summary>What the hold trigger reads from the system and the effects that it causes.</summary>
public interface IHoldTriggerHost
{
    void StartTimer(TimeSpan delay);
    void CancelTimer();
    Point2 Pointer { get; }
    bool AnyMouseButtonDown { get; }
    /// <summary>True when a window of Insert has the keyboard, for example the tray or Settings.</summary>
    bool OwnWindowIsActive { get; }
    /// <summary>True when a key that is not a modifier is down. Such a key sends no key press during the wait.</summary>
    bool OtherKeyIsDown { get; }
    /// <summary>True when another app shows a menu. A hold while a menu is open is for that menu.</summary>
    bool MenuIsOpen { get; }
    ModifierKeys CurrentModifiers { get; }
    /// <summary>Opens the ring. Returns false when the ring cannot open, for example with no clips.</summary>
    bool Open();
    /// <summary>The user released the modifier while the ring was open.</summary>
    void Release();
}

public enum HoldState
{
    Idle,
    Waiting,
    Holding,
}

/// <summary>
/// Opens the clip ring when the user holds one modifier key alone for <see cref="Delay"/>.
/// The keyboard hook calls <see cref="ModifiersChanged"/> and <see cref="Interrupted"/>. The host timer calls <see cref="TimerFired"/>.
/// </summary>
public sealed class HoldTrigger(IHoldTriggerHost host)
{
    public static readonly TimeSpan Delay = TimeSpan.FromSeconds(0.3);
    public const double PointerTolerance = 10;

    private ModifierKeys _held;
    private Point2 _start;

    public RingModifier Modifier { get; set; } = RingModifier.Alt;
    public HoldState State { get; private set; } = HoldState.Idle;

    /// <summary>A key that is not a modifier went down, or a mouse button went down, or the wheel turned.</summary>
    public void Interrupted() => CancelWait();

    public void ModifiersChanged(ModifierKeys held)
    {
        var wasEmpty = _held == ModifierKeys.None;
        _held = held;
        switch (State)
        {
            case HoldState.Idle:
                var isLonePress = wasEmpty && held == Modifier.Key();
                if (isLonePress && !host.AnyMouseButtonDown && !host.OtherKeyIsDown && !host.OwnWindowIsActive) StartWaiting();
                break;
            case HoldState.Waiting:
                if (held != Modifier.Key()) CancelWait();
                break;
            case HoldState.Holding:
                if (!held.HasFlag(Modifier.Key()))
                {
                    State = HoldState.Idle;
                    host.Release();
                }
                break;
        }
    }

    public void TimerFired()
    {
        if (State != HoldState.Waiting) return;
        var isStill = host.Pointer.DistanceTo(_start) <= PointerTolerance;
        var isAlone = host.CurrentModifiers == Modifier.Key() && !host.AnyMouseButtonDown && !host.OtherKeyIsDown;
        var canOpen = isStill && isAlone && !host.MenuIsOpen;
        State = canOpen && host.Open() ? HoldState.Holding : HoldState.Idle;
    }

    /// <summary>Reads the modifiers again, for example after the keyboard hook was off.</summary>
    public void Resync()
    {
        CancelWait();
        _held = host.CurrentModifiers;
        if (State == HoldState.Holding && !_held.HasFlag(Modifier.Key()))
        {
            State = HoldState.Idle;
            host.Release();
        }
    }

    private void StartWaiting()
    {
        _start = host.Pointer;
        State = HoldState.Waiting;
        host.StartTimer(Delay);
    }

    private void CancelWait()
    {
        if (State != HoldState.Waiting) return;
        host.CancelTimer();
        State = HoldState.Idle;
    }
}
