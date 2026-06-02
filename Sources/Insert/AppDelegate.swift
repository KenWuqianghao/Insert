import AppKit
import Combine

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private let clipboardStore = ClipboardStore()
    private let preferences = Preferences()
    private let hotKeyManager = HotKeyManager()

    private var panelController: PanelController?
    private var statusItem: NSStatusItem?
    private var cancellables = Set<AnyCancellable>()

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.regular)
        preferences.applyDockPolicy()

        clipboardStore.start()

        let panelController = PanelController(clipboardStore: clipboardStore, preferences: preferences)
        self.panelController = panelController

        configureStatusItem()

        registerHotKey()

        preferences.$hotKeyShortcut
            .dropFirst()
            .sink { [weak self] _ in
                self?.registerHotKey()
            }
            .store(in: &cancellables)
    }

    func applicationWillTerminate(_ notification: Notification) {
        clipboardStore.stop()
        hotKeyManager.unregister()
    }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        panelController?.show()
        return true
    }

    private func configureStatusItem() {
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        item.button?.image = NSImage(systemSymbolName: "doc.on.clipboard", accessibilityDescription: "Insert")

        let menu = NSMenu()
        menu.addItem(NSMenuItem(title: "Show Insert", action: #selector(showPanel), keyEquivalent: ""))
        menu.addItem(NSMenuItem.separator())
        menu.addItem(NSMenuItem(title: "Quit Insert", action: #selector(quit), keyEquivalent: "q"))
        item.menu = menu

        statusItem = item
    }

    @objc private func showPanel() {
        panelController?.show()
    }

    @objc private func quit() {
        NSApp.terminate(nil)
    }

    private func registerHotKey() {
        hotKeyManager.register(shortcut: preferences.hotKeyShortcut) { [weak self] in
            self?.panelController?.toggle()
        }
    }
}
