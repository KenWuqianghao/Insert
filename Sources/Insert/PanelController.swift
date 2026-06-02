import AppKit
import Carbon
import SwiftUI

@MainActor
final class PanelController {
    private let clipboardStore: ClipboardStore
    private let preferences: Preferences
    private lazy var panel = InsertPanel()
    private var keyEventMonitor: Any?
    private var shortcutRecordingObserver: NSObjectProtocol?
    private var isRecordingShortcut = false

    private var visibleFrame: NSRect = .zero
    private var hiddenFrame: NSRect = .zero

    var isVisible: Bool {
        panel.isVisible
    }

    init(clipboardStore: ClipboardStore, preferences: Preferences) {
        self.clipboardStore = clipboardStore
        self.preferences = preferences
        configurePanel()
    }

    func toggle() {
        isVisible ? hide() : show()
    }

    func show() {
        configureFrames()
        panel.setFrame(hiddenFrame, display: false)
        panel.alphaValue = 0
        panel.orderFrontRegardless()
        NSApp.activate(ignoringOtherApps: true)
        panel.makeKey()
        NotificationCenter.default.post(name: .insertPanelWillShow, object: nil)

        NSAnimationContext.runAnimationGroup { context in
            context.duration = 0.24
            context.timingFunction = CAMediaTimingFunction(name: .easeOut)
            panel.animator().setFrame(visibleFrame, display: true)
            panel.animator().alphaValue = 1
        }
    }

    func hide() {
        guard panel.isVisible else { return }

        NSAnimationContext.runAnimationGroup { context in
            context.duration = 0.18
            context.timingFunction = CAMediaTimingFunction(name: .easeIn)
            panel.animator().setFrame(hiddenFrame, display: true)
            panel.animator().alphaValue = 0
        } completionHandler: { [weak self] in
            Task { @MainActor in
                self?.panel.orderOut(nil)
            }
        }
    }

    private func configurePanel() {
        panel.onEscape = { [weak self] in
            self?.hide()
        }
        panel.onKeyCommand = { command in
            NotificationCenter.default.post(name: .insertKeyCommand, object: command)
            return true
        }

        panel.styleMask = [.borderless, .fullSizeContentView]
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = false
        panel.level = .floating
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .transient]
        panel.isReleasedWhenClosed = false

        let rootView = RootView(
            clipboardStore: clipboardStore,
            preferences: preferences,
            onHide: { [weak self] in self?.hide() },
            onSelect: { [weak self] item in
                self?.clipboardStore.copyToPasteboard(item)
                self?.hide()
            }
        )

        panel.contentView = NSHostingView(rootView: rootView)

        keyEventMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            guard
                let self,
                self.panel.isVisible,
                self.panel.isKeyWindow
            else {
                return event
            }

            if self.isRecordingShortcut {
                self.handleShortcutRecordingEvent(event)
                return nil
            }

            if let command = InsertKeyCommand(event: event) {
                if command == .deleteSelection, self.searchFieldEditorHasText() {
                    return event
                }

                if self.panel.onKeyCommand?(command) == true {
                    return nil
                }
            }

            return event
        }

        shortcutRecordingObserver = NotificationCenter.default.addObserver(
            forName: .insertShortcutRecordingChanged,
            object: nil,
            queue: .main
        ) { [weak self] notification in
            Task { @MainActor in
                self?.isRecordingShortcut = (notification.object as? Bool) ?? false
            }
        }
    }

    private func searchFieldEditorHasText() -> Bool {
        guard let fieldEditor = panel.fieldEditor(false, for: nil) as? NSTextView else { return false }
        return !fieldEditor.string.isEmpty
    }

    private func handleShortcutRecordingEvent(_ event: NSEvent) {
        if Int(event.keyCode) == kVK_Escape {
            NotificationCenter.default.post(name: .insertShortcutRecordingCancelled, object: nil)
            return
        }

        guard let shortcut = GlobalShortcut(event: event) else {
            NSSound.beep()
            return
        }

        NotificationCenter.default.post(name: .insertRecordedShortcut, object: shortcut)
    }

    private func configureFrames() {
        let screen = NSScreen.main ?? NSScreen.screens.first
        let screenFrame = screen?.visibleFrame ?? NSRect(x: 0, y: 0, width: 1200, height: 800)
        let horizontalMargin: CGFloat = 28
        let height = min(max(screenFrame.height * 0.34, 268), 360)
        let width = max(screenFrame.width - horizontalMargin * 2, 720)
        let x = screenFrame.midX - width / 2
        let y = screenFrame.minY + 18

        visibleFrame = NSRect(x: x, y: y, width: width, height: height)
        hiddenFrame = NSRect(x: x, y: screenFrame.minY - height - 36, width: width, height: height)
    }
}

final class InsertPanel: NSPanel {
    var onEscape: (() -> Void)?
    var onKeyCommand: ((InsertKeyCommand) -> Bool)?

    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { true }

    override func cancelOperation(_ sender: Any?) {
        onEscape?()
    }

    override func keyDown(with event: NSEvent) {
        if let command = InsertKeyCommand(event: event), onKeyCommand?(command) == true {
            return
        }

        super.keyDown(with: event)
    }
}

enum InsertKeyCommand: Equatable {
    case movePrevious
    case moveNext
    case copySelection
    case deleteSelection

    init?(event: NSEvent) {
        let commandPressed = event.modifierFlags.intersection(.deviceIndependentFlagsMask).contains(.command)

        if commandPressed, event.charactersIgnoringModifiers?.lowercased() == "c" {
            self = .copySelection
            return
        }

        guard !commandPressed else { return nil }

        switch Int(event.keyCode) {
        case kVK_LeftArrow, kVK_UpArrow:
            self = .movePrevious
        case kVK_RightArrow, kVK_DownArrow:
            self = .moveNext
        case kVK_Return, kVK_ANSI_KeypadEnter:
            self = .copySelection
        case kVK_Delete, kVK_ForwardDelete:
            self = .deleteSelection
        default:
            return nil
        }
    }
}

extension Notification.Name {
    static let insertPanelWillShow = Notification.Name("insertPanelWillShow")
    static let insertKeyCommand = Notification.Name("insertKeyCommand")
    static let insertRecordedShortcut = Notification.Name("insertRecordedShortcut")
    static let insertShortcutRecordingChanged = Notification.Name("insertShortcutRecordingChanged")
    static let insertShortcutRecordingCancelled = Notification.Name("insertShortcutRecordingCancelled")
}
