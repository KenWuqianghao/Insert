import AppKit
import Carbon

enum PasteAction: Equatable {
    case paste
    case pastePlainText
    case copy
}

enum PanelCommand: Equatable {
    case move(Int)
    case extend(Int)
    case selectAll
    case activate(PasteAction)
    case activateIndex(Int)
    case deleteSelection
    case togglePreview
    case cancel
    case switchScope(Int)
    case openSettings
    case hide

    struct Context {
        let mode: PanelModel.Mode
        let queryIsEmpty: Bool
    }

    init?(event: NSEvent, context: Context) {
        let modifiers = event.modifierFlags.intersection([.command, .shift, .option, .control])
        let character = event.charactersIgnoringModifiers?.lowercased()
        let key = Int(event.keyCode)

        // The tray is key while another app is active. Cmd+Q must not quit Insert by accident.
        if modifiers == .command, character == "q" || character == "w" {
            self = .hide
            return
        }

        if case .naming = context.mode {
            guard key == kVK_Escape else { return nil }
            self = .cancel
            return
        }

        if modifiers == .command {
            switch (character, key) {
            case ("c", _): self = .activate(.copy)
            case ("a", _): self = .selectAll
            case (",", _): self = .openSettings
            case ("[", _): self = .switchScope(-1)
            case ("]", _): self = .switchScope(1)
            case (_, kVK_Delete), (_, kVK_ForwardDelete): self = .deleteSelection
            case (let digit?, _):
                guard let number = Int(digit), (1...9).contains(number) else { return nil }
                self = .activateIndex(number - 1)
            default: return nil
            }
            return
        }

        let isPlain = modifiers.isEmpty
        let isShift = modifiers == .shift
        guard isPlain || isShift else { return nil }

        switch key {
        case kVK_LeftArrow, kVK_UpArrow:
            self = isShift ? .extend(-1) : .move(-1)
        case kVK_RightArrow, kVK_DownArrow:
            self = isShift ? .extend(1) : .move(1)
        case kVK_Return, kVK_ANSI_KeypadEnter:
            self = .activate(isShift ? .pastePlainText : .paste)
        case kVK_Tab:
            self = .switchScope(isShift ? -1 : 1)
        case kVK_Escape where isPlain:
            self = .cancel
        case kVK_Delete where isPlain && context.queryIsEmpty,
             kVK_ForwardDelete where isPlain && context.queryIsEmpty:
            self = .deleteSelection
        case kVK_Space where isPlain && (context.queryIsEmpty || context.mode == .previewing):
            self = .togglePreview
        default:
            return nil
        }
    }
}
