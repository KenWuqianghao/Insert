import AppKit
import ApplicationServices
import Carbon
import Combine

@MainActor
final class PasteService {
    private let library: ClipLibrary
    private let monitor: ClipboardMonitor
    private let preferences: Preferences
    private var lastExternalApp: NSRunningApplication?
    private var cancellables = Set<AnyCancellable>()

    init(library: ClipLibrary, monitor: ClipboardMonitor, preferences: Preferences) {
        self.library = library
        self.monitor = monitor
        self.preferences = preferences

        NSWorkspace.shared.notificationCenter
            .publisher(for: NSWorkspace.didActivateApplicationNotification)
            .compactMap { $0.userInfo?[NSWorkspace.applicationUserInfoKey] as? NSRunningApplication }
            .filter { $0.processIdentifier != ProcessInfo.processInfo.processIdentifier }
            .sink { [weak self] app in
                self?.lastExternalApp = app
            }
            .store(in: &cancellables)
    }

    /// Posting a keystroke to another app needs the Accessibility permission.
    var needsPermission: Bool {
        preferences.pasteDirectly && !AXIsProcessTrusted()
    }

    /// Returns false when the payload file of the clip is missing.
    func perform(_ action: PasteAction, with clip: Clip) -> Bool {
        guard let payload = library.payload(for: clip.id) else { return false }

        monitor.write { pasteboard in
            if action == .pastePlainText, let text = payload.plainText {
                PasteboardWriter.write(plainText: text, to: pasteboard)
            } else {
                PasteboardWriter.write(payload, to: pasteboard)
            }
        }
        library.moveToFront(clip.id)

        if action != .copy, preferences.pasteDirectly, AXIsProcessTrusted() {
            pasteIntoActiveApp()
        }
        return true
    }

    func requestPermission() {
        let options = [kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: true] as CFDictionary
        AXIsProcessTrustedWithOptions(options)

        if let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility") {
            NSWorkspace.shared.open(url)
        }
    }

    private func pasteIntoActiveApp() {
        // The tray does not activate Insert, so the target app is usually still active.
        // Insert is active only when the user opened the tray from the Dock or from a window of Insert.
        guard NSApp.isActive else {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1, execute: { Self.postCommandV() })
            return
        }

        lastExternalApp?.activate()
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.18, execute: { Self.postCommandV() })
    }

    private static func postCommandV() {
        let source = CGEventSource(stateID: .combinedSessionState)
        source?.setLocalEventsFilterDuringSuppressionState(
            [.permitLocalMouseEvents, .permitSystemDefinedEvents],
            state: .eventSuppressionStateSuppressionInterval
        )

        for keyDown in [true, false] {
            let event = CGEvent(keyboardEventSource: source, virtualKey: CGKeyCode(kVK_ANSI_V), keyDown: keyDown)
            event?.flags = .maskCommand
            event?.post(tap: .cgSessionEventTap)
        }
    }
}
