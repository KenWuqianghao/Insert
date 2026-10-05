import AppKit
import Carbon
import Foundation

struct GlobalShortcut: Equatable {
    let keyCode: UInt32
    let carbonModifiers: UInt32
    let displayName: String

    static let defaultShortcut = GlobalShortcut(
        keyCode: UInt32(kVK_ANSI_V),
        carbonModifiers: UInt32(cmdKey | shiftKey),
        displayName: "⇧⌘V"
    )

    init(keyCode: UInt32, carbonModifiers: UInt32, displayName: String) {
        self.keyCode = keyCode
        self.carbonModifiers = carbonModifiers
        self.displayName = displayName
    }

    init?(event: NSEvent) {
        let carbonModifiers = Self.carbonModifiers(from: event.modifierFlags)
        guard carbonModifiers != 0 else { return nil }

        let keyCode = UInt32(event.keyCode)
        let keyName = Self.keyName(for: event)
        guard !keyName.isEmpty else { return nil }

        self.keyCode = keyCode
        self.carbonModifiers = carbonModifiers
        self.displayName = Self.displayName(carbonModifiers: carbonModifiers, keyName: keyName)
    }

    init(defaults: UserDefaults) {
        let keyCode = defaults.object(forKey: Keys.keyCode) as? Int
        let modifiers = defaults.object(forKey: Keys.modifiers) as? Int
        let displayName = defaults.string(forKey: Keys.displayName)

        guard
            let keyCode,
            let modifiers,
            let displayName,
            keyCode >= 0,
            modifiers > 0,
            !displayName.isEmpty
        else {
            self = .defaultShortcut
            return
        }

        self.keyCode = UInt32(keyCode)
        self.carbonModifiers = UInt32(modifiers)
        self.displayName = displayName
    }

    func save(to defaults: UserDefaults) {
        defaults.set(Int(keyCode), forKey: Keys.keyCode)
        defaults.set(Int(carbonModifiers), forKey: Keys.modifiers)
        defaults.set(displayName, forKey: Keys.displayName)
    }

    private static func carbonModifiers(from flags: NSEvent.ModifierFlags) -> UInt32 {
        let normalized = flags.intersection(.deviceIndependentFlagsMask)
        var modifiers: UInt32 = 0

        if normalized.contains(.command) {
            modifiers |= UInt32(cmdKey)
        }

        if normalized.contains(.shift) {
            modifiers |= UInt32(shiftKey)
        }

        if normalized.contains(.option) {
            modifiers |= UInt32(optionKey)
        }

        if normalized.contains(.control) {
            modifiers |= UInt32(controlKey)
        }

        return modifiers
    }

    private static func displayName(carbonModifiers: UInt32, keyName: String) -> String {
        var parts = ""

        if carbonModifiers & UInt32(controlKey) != 0 {
            parts += "⌃"
        }

        if carbonModifiers & UInt32(optionKey) != 0 {
            parts += "⌥"
        }

        if carbonModifiers & UInt32(shiftKey) != 0 {
            parts += "⇧"
        }

        if carbonModifiers & UInt32(cmdKey) != 0 {
            parts += "⌘"
        }

        return parts + keyName
    }

    private static func keyName(for event: NSEvent) -> String {
        switch Int(event.keyCode) {
        case kVK_Space:
            return "Space"
        case kVK_Return:
            return "Return"
        case kVK_Tab:
            return "Tab"
        case kVK_Escape:
            return "Esc"
        case kVK_LeftArrow:
            return "←"
        case kVK_RightArrow:
            return "→"
        case kVK_UpArrow:
            return "↑"
        case kVK_DownArrow:
            return "↓"
        default:
            return event.charactersIgnoringModifiers?
                .trimmingCharacters(in: .whitespacesAndNewlines)
                .uppercased() ?? ""
        }
    }

    private enum Keys {
        static let keyCode = "hotKey.keyCode"
        static let modifiers = "hotKey.modifiers"
        static let displayName = "hotKey.displayName"
    }
}
