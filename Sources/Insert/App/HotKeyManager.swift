import Carbon
import Foundation

@MainActor
final class HotKeyManager {
    enum Slot: UInt32, CaseIterable {
        case tray = 1
        case ring = 2
    }

    struct Actions {
        var pressed: () -> Void
        var released: () -> Void = {}
    }

    private var hotKeyRefs: [Slot: EventHotKeyRef] = [:]
    private var actions: [Slot: Actions] = [:]
    private var eventHandlerRef: EventHandlerRef?

    @discardableResult
    func register(_ shortcut: GlobalShortcut, for slot: Slot, actions: Actions) -> OSStatus {
        unregister(slot)

        let handlerStatus = installEventHandler()
        guard handlerStatus == noErr else { return handlerStatus }

        var hotKeyRef: EventHotKeyRef?
        let status = RegisterEventHotKey(
            shortcut.keyCode,
            shortcut.carbonModifiers,
            EventHotKeyID(signature: hotKeySignature, id: slot.rawValue),
            GetApplicationEventTarget(),
            0,
            &hotKeyRef
        )
        if status == noErr, let hotKeyRef {
            hotKeyRefs[slot] = hotKeyRef
            self.actions[slot] = actions
        }
        return status
    }

    func unregisterAll() {
        Slot.allCases.forEach(unregister)

        if let eventHandlerRef {
            RemoveEventHandler(eventHandlerRef)
            self.eventHandlerRef = nil
        }
    }

    private func unregister(_ slot: Slot) {
        if let hotKeyRef = hotKeyRefs.removeValue(forKey: slot) {
            UnregisterEventHotKey(hotKeyRef)
        }
        actions[slot] = nil
    }

    private func installEventHandler() -> OSStatus {
        guard eventHandlerRef == nil else { return noErr }

        let eventTypes = [
            EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed)),
            EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyReleased))
        ]
        return InstallEventHandler(
            GetApplicationEventTarget(),
            hotKeyEventHandler,
            eventTypes.count,
            eventTypes,
            Unmanaged.passUnretained(self).toOpaque(),
            &eventHandlerRef
        )
    }

    fileprivate func handle(id: UInt32, isPressed: Bool) {
        guard let slot = Slot(rawValue: id), let actions = actions[slot] else { return }
        isPressed ? actions.pressed() : actions.released()
    }
}

// The application event target delivers its events on the main thread.
private let hotKeyEventHandler: EventHandlerUPP = { _, event, userData in
    guard let event, let userData else { return noErr }

    var hotKeyID = EventHotKeyID()
    let status = GetEventParameter(
        event,
        EventParamName(kEventParamDirectObject),
        EventParamType(typeEventHotKeyID),
        nil,
        MemoryLayout<EventHotKeyID>.size,
        nil,
        &hotKeyID
    )
    guard status == noErr, hotKeyID.signature == hotKeySignature else { return noErr }

    let isPressed = GetEventKind(event) == UInt32(kEventHotKeyPressed)
    let manager = Unmanaged<HotKeyManager>.fromOpaque(userData).takeUnretainedValue()
    MainActor.assumeIsolated {
        manager.handle(id: hotKeyID.id, isPressed: isPressed)
    }
    return noErr
}

private let hotKeySignature = fourCharCode("INSR")

private func fourCharCode(_ string: String) -> OSType {
    string.utf8.reduce(0) { result, character in
        (result << 8) + OSType(character)
    }
}
