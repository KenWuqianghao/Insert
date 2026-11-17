import AppKit
import Carbon

enum RingModifier: String, CaseIterable, Identifiable {
    case option
    case command
    case control

    var id: Self { self }

    var flag: NSEvent.ModifierFlags {
        switch self {
        case .option: .option
        case .command: .command
        case .control: .control
        }
    }

    var name: String {
        switch self {
        case .option: "⌥ Option"
        case .command: "⌘ Command"
        case .control: "⌃ Control"
        }
    }
}

@MainActor
final class ModifierHoldTrigger {
    private enum State {
        case idle
        case waiting(pointer: CGPoint, timer: DispatchWorkItem)
        case holding
    }

    private static let delay: TimeInterval = 0.3
    private static let pointerTolerance: CGFloat = 10
    private static let modifierMask: NSEvent.ModifierFlags = [.command, .option, .control, .shift, .function]
    /// From kVK_RightCommand to kVK_Function, including Caps Lock.
    private static let modifierKeyCodes = 0x36...0x3F
    /// Media and brightness keys come as system-defined events. CGEventType has no case for them.
    private static let systemDefined = CGEventType(rawValue: 14)!
    private static let interruptions: [CGEventType] = [
        .keyDown, .leftMouseDown, .rightMouseDown, .otherMouseDown, .scrollWheel, systemDefined
    ]
    private static let eventMask = (interruptions + [.flagsChanged]).reduce(CGEventMask(0)) { $0 | (1 << $1.rawValue) }

    var modifier = RingModifier.option

    private let onHold: () -> Bool
    private let onRelease: () -> Void
    private var state = State.idle
    private var held: NSEvent.ModifierFlags = []
    private var tap: CFMachPort?
    private var tapSource: CFRunLoopSource?

    init(onHold: @escaping () -> Bool, onRelease: @escaping () -> Void) {
        self.onHold = onHold
        self.onRelease = onRelease

        updateTap()
        // macOS posts this notification when an app gets or loses the Accessibility permission.
        // The new state is readable a moment later.
        DistributedNotificationCenter.default().addObserver(
            forName: NSNotification.Name("com.apple.accessibility.api"),
            object: nil,
            queue: .main
        ) { [weak self] _ in
            DispatchQueue.main.asyncAfter(deadline: .now() + 1) {
                MainActor.assumeIsolated {
                    self?.updateTap()
                }
            }
        }
        NotificationCenter.default.addObserver(forName: NSApplication.didBecomeActiveNotification, object: nil, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated {
                self?.updateTap()
            }
        }
    }

    /// An NSEvent global monitor does not see keys that the system takes, such as the Tab of Command+Tab. An event tap sees them.
    /// Without the Accessibility permission, a key tap makes macOS ask for Input Monitoring, so no tap is made then.
    /// macOS can stop a tap when the permission changes, so a tap that does not run is made again.
    private func updateTap() {
        let isTrusted = AXIsProcessTrusted()
        if let tap, isTrusted, CGEvent.tapIsEnabled(tap: tap) { return }
        removeTap()
        guard isTrusted, let tap = CGEvent.tapCreate(
            tap: .cgSessionEventTap,
            place: .headInsertEventTap,
            options: .listenOnly,
            eventsOfInterest: Self.eventMask,
            callback: { _, type, event, userInfo in
                if let userInfo {
                    let trigger = Unmanaged<ModifierHoldTrigger>.fromOpaque(userInfo).takeUnretainedValue()
                    MainActor.assumeIsolated {
                        trigger.handle(type, event)
                    }
                }
                return Unmanaged.passUnretained(event)
            },
            userInfo: Unmanaged.passUnretained(self).toOpaque()
        ) else { return }

        let source = CFMachPortCreateRunLoopSource(nil, tap, 0)
        CFRunLoopAddSource(CFRunLoopGetMain(), source, .commonModes)
        self.tap = tap
        tapSource = source
        resync()
    }

    private func removeTap() {
        cancelWait()
        if let tap {
            CFMachPortInvalidate(tap)
        }
        if let tapSource {
            CFRunLoopRemoveSource(CFRunLoopGetMain(), tapSource, .commonModes)
        }
        tap = nil
        tapSource = nil
    }

    private func handle(_ type: CGEventType, _ event: CGEvent) {
        switch type {
        case .flagsChanged:
            modifiersChanged(NSEvent.ModifierFlags(rawValue: UInt(event.flags.rawValue)))
        case .tapDisabledByTimeout, .tapDisabledByUserInput:
            if let tap {
                CGEvent.tapEnable(tap: tap, enable: true)
            }
            resync()
        default:
            // The tap also gets null events, for example after a modifier press.
            if Self.interruptions.contains(type) {
                cancelWait()
            }
        }
    }

    private func resync() {
        cancelWait()
        held = NSEvent.modifierFlags.intersection(Self.modifierMask)
        if case .holding = state, !held.contains(modifier.flag) {
            state = .idle
            onRelease()
        }
    }

    private func modifiersChanged(_ flags: NSEvent.ModifierFlags) {
        let wasEmpty = held.isEmpty
        held = flags.intersection(Self.modifierMask)
        switch state {
        case .idle:
            let isLonePress = wasEmpty && held == modifier.flag
            if isLonePress, NSEvent.pressedMouseButtons == 0, NSApp.keyWindow == nil {
                startWaiting()
            }
        case .waiting:
            if held != modifier.flag {
                cancelWait()
            }
        case .holding:
            if !held.contains(modifier.flag) {
                state = .idle
                onRelease()
            }
        }
    }

    private func startWaiting() {
        let timer = DispatchWorkItem { [weak self] in
            MainActor.assumeIsolated {
                self?.waitEnded()
            }
        }
        state = .waiting(pointer: NSEvent.mouseLocation, timer: timer)
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.delay, execute: timer)
    }

    private func cancelWait() {
        guard case .waiting(_, let timer) = state else { return }
        timer.cancel()
        state = .idle
    }

    private func waitEnded() {
        guard case .waiting(let start, _) = state else { return }
        let pointer = NSEvent.mouseLocation
        let isStill = hypot(pointer.x - start.x, pointer.y - start.y) <= Self.pointerTolerance
        let isAlone = NSEvent.modifierFlags.intersection(Self.modifierMask) == modifier.flag
            && NSEvent.pressedMouseButtons == 0
            && !Self.otherKeyIsDown()
        // In secure input, such as a password field, the tap gets no key presses.
        let canOpen = isStill && isAlone && !IsSecureEventInputEnabled() && !Self.menuIsOpen()
        state = canOpen && onHold() ? .holding : .idle
    }

    /// A key that was down before the modifier sends no key press during the wait.
    private static func otherKeyIsDown() -> Bool {
        (0..<128).contains { code in
            !modifierKeyCodes.contains(code) && CGEventSource.keyState(.combinedSessionState, key: CGKeyCode(code))
        }
    }

    /// Some apps keep a clear window on the menu level at all times, so only visible windows count.
    private static func isVisible(_ window: [String: Any]) -> Bool {
        (window[kCGWindowAlpha as String] as? Double ?? 0) > 0
    }

    /// Option shows other items in an open menu, so a hold while a menu is open is for that menu.
    private static func menuIsOpen() -> Bool {
        let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] ?? []
        let menuLevel = Int(CGWindowLevelForKey(.popUpMenuWindow))
        let ownPID = ProcessInfo.processInfo.processIdentifier
        return windows.contains {
            $0[kCGWindowLayer as String] as? Int == menuLevel
                && $0[kCGWindowOwnerPID as String] as? Int32 != ownPID
                && isVisible($0)
        }
    }
}
