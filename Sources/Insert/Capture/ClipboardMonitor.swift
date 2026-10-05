import AppKit

@MainActor
final class ClipboardMonitor {
    private let library: ClipLibrary
    private let preferences: Preferences
    private let pasteboard = NSPasteboard.general
    private var changeCount = NSPasteboard.general.changeCount
    private var timer: Timer?

    init(library: ClipLibrary, preferences: Preferences) {
        self.library = library
        self.preferences = preferences
    }

    func start() {
        guard timer == nil else { return }
        let timer = Timer(timeInterval: 0.5, repeats: true) { [weak self] _ in
            Task { @MainActor in
                self?.poll()
            }
        }
        RunLoop.main.add(timer, forMode: .common)
        self.timer = timer
    }

    func stop() {
        timer?.invalidate()
        timer = nil
    }

    /// Insert writes to the pasteboard only through this method, so the monitor does not capture the write.
    func write(_ body: (NSPasteboard) -> Void) {
        body(pasteboard)
        changeCount = pasteboard.changeCount
    }

    private func poll() {
        guard pasteboard.changeCount != changeCount else { return }
        changeCount = pasteboard.changeCount
        guard !preferences.capturePaused else { return }

        let source = NSWorkspace.shared.frontmostApplication.flatMap { app in
            app.bundleIdentifier.map { SourceApp(bundleID: $0, name: app.localizedName ?? $0) }
        }
        if let source, preferences.ignoredApps.contains(where: { $0.bundleID == source.bundleID }) {
            return
        }

        guard let capture = PasteboardReader.read(pasteboard, source: source) else { return }
        library.insert(capture)
    }
}
