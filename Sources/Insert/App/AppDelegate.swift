import AppKit
import Carbon
import Combine

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate, NSMenuDelegate {
    private let preferences = Preferences()
    private let licenseStore = LicenseStore()
    private let hotKeyManager = HotKeyManager()

    private var library: ClipLibrary?
    private var monitor: ClipboardMonitor?
    private var panelController: PanelController?
    private var settingsController: SettingsWindowController?
    private var statusItem: NSStatusItem?
    private var pauseMenuItem: NSMenuItem?
    private var buyMenuItem: NSMenuItem?
    private var buySeparator: NSMenuItem?
    private var cancellables = Set<AnyCancellable>()

    func applicationDidFinishLaunching(_ notification: Notification) {
        preferences.applyDockPolicy()

        // The bundle id names the storage folder, so two builds with different ids never share data.
        let bundleID = Bundle.main.bundleIdentifier ?? ProcessInfo.processInfo.processName
        let library = ClipLibrary(storage: ClipStorage(bundleID: bundleID), historyLimit: preferences.historyLimit)
        let monitor = ClipboardMonitor(library: library, preferences: preferences)
        let pasteService = PasteService(library: library, monitor: monitor, preferences: preferences)
        let model = PanelModel(library: library)
        let panelController = PanelController(model: model, preferences: preferences, pasteService: pasteService)
        let settingsController = SettingsWindowController(
            preferences: preferences,
            licenseStore: licenseStore,
            actions: .init(
                clearHistory: { [weak self] in self?.confirmClearHistory() },
                requestPastePermission: { pasteService.requestPermission() }
            )
        )

        model.actions.showSettings = { [weak self] in self?.showSettings() }
        model.actions.clearHistory = { [weak self] in self?.confirmClearHistory() }

        self.library = library
        self.monitor = monitor
        self.panelController = panelController
        self.settingsController = settingsController

        configureStatusItem()
        monitor.start()

        preferences.$hotKeyShortcut
            .sink { [weak self] shortcut in
                self?.registerHotKey(shortcut)
            }
            .store(in: &cancellables)

        preferences.$historyLimit
            .dropFirst()
            .sink { library.historyLimit = $0 }
            .store(in: &cancellables)

        preferences.$capturePaused
            .sink { [weak self] isPaused in
                self?.showCaptureState(isPaused: isPaused)
            }
            .store(in: &cancellables)

        observeDockPolicyRestoreEvents()
    }

    func applicationWillTerminate(_ notification: Notification) {
        monitor?.stop()
        library?.flush()
        hotKeyManager.unregister()
    }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        openTray()
        return true
    }

    func application(_ application: NSApplication, open urls: [URL]) {
        for url in urls where url.scheme == "insert" && url.host == "activate" {
            licenseStore.activate(url: url)
            // A link can launch the app. Then this runs before applicationDidFinishLaunching makes the Settings window.
            DispatchQueue.main.async { [weak self] in
                self?.showLicense()
            }
        }
    }

    @objc func showSettings() {
        panelController?.hide()
        settingsController?.show()
    }

    @objc private func showLicense() {
        panelController?.hide()
        settingsController?.show(.license)
    }

    /// After the trial, the tray stays closed until the user activates a license. Capture continues, so no clip is lost.
    private func openTray(toggle: Bool = false) {
        guard licenseStore.state.opensTray else {
            showLicense()
            return
        }
        toggle ? panelController?.toggle() : panelController?.show()
    }

    private func configureStatusItem() {
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)

        let pauseItem = NSMenuItem(title: "", action: #selector(toggleCapture), keyEquivalent: "")
        let buyItem = NSMenuItem(title: "", action: #selector(showLicense), keyEquivalent: "")
        let buySeparator = NSMenuItem.separator()
        let menu = NSMenu()
        menu.delegate = self
        menu.addItem(buyItem)
        menu.addItem(buySeparator)
        menu.addItem(NSMenuItem(title: "Show Insert", action: #selector(showPanel), keyEquivalent: ""))
        menu.addItem(pauseItem)
        menu.addItem(NSMenuItem(title: "Clear History…", action: #selector(confirmClearHistory), keyEquivalent: ""))
        menu.addItem(NSMenuItem.separator())
        menu.addItem(NSMenuItem(title: "Settings…", action: #selector(showSettings), keyEquivalent: ","))
        menu.addItem(NSMenuItem.separator())
        menu.addItem(NSMenuItem(title: "Quit Insert", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q"))
        for menuItem in menu.items where menuItem.action != #selector(NSApplication.terminate(_:)) {
            menuItem.target = self
        }
        item.menu = menu

        statusItem = item
        pauseMenuItem = pauseItem
        buyMenuItem = buyItem
        self.buySeparator = buySeparator
    }

    func menuNeedsUpdate(_ menu: NSMenu) {
        let buyTitle: String?
        switch licenseStore.state {
        case .trial(let daysLeft):
            buyTitle = daysLeft == 1 ? "Buy Insert (1 day left)…" : "Buy Insert (\(daysLeft) days left)…"
        case .trialEnded:
            buyTitle = "Buy Insert…"
        case .licensed:
            buyTitle = nil
        }
        buyMenuItem?.title = buyTitle ?? ""
        buyMenuItem?.isHidden = buyTitle == nil
        buySeparator?.isHidden = buyTitle == nil
    }

    private func showCaptureState(isPaused: Bool) {
        pauseMenuItem?.title = isPaused ? "Resume Capture" : "Pause Capture"
        statusItem?.button?.image = NSImage(
            systemSymbolName: isPaused ? "pause.circle" : "doc.on.clipboard",
            accessibilityDescription: isPaused ? "Insert, capture is paused" : "Insert"
        )
    }

    @objc private func showPanel() {
        openTray()
    }

    @objc private func toggleCapture() {
        preferences.capturePaused.toggle()
    }

    @objc private func confirmClearHistory() {
        panelController?.hide()
        NSApp.activate(ignoringOtherApps: true)

        let alert = NSAlert()
        alert.alertStyle = .warning
        alert.messageText = "Clear the clipboard history?"
        alert.informativeText = "Insert deletes all clips that are not in a pinboard. You cannot undo this."
        alert.addButton(withTitle: "Clear History").hasDestructiveAction = true
        alert.addButton(withTitle: "Cancel")

        if alert.runModal() == .alertFirstButtonReturn {
            library?.clearHistory()
        }
    }

    private func registerHotKey(_ shortcut: GlobalShortcut) {
        let status = hotKeyManager.register(shortcut: shortcut) { [weak self] in
            self?.openTray(toggle: true)
        }
        let isUnavailable = status != noErr
        if preferences.hotKeyIsUnavailable != isUnavailable {
            preferences.hotKeyIsUnavailable = isUnavailable
        }
    }

    private func observeDockPolicyRestoreEvents() {
        NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)
            .sink { [weak self] _ in
                self?.preferences.applyDockPolicy()
            }
            .store(in: &cancellables)

        NSWorkspace.shared.notificationCenter.publisher(for: NSWorkspace.didWakeNotification)
            .sink { [weak self] _ in
                self?.preferences.applyDockPolicy()
            }
            .store(in: &cancellables)
    }
}
