import AppKit
import Foundation

@MainActor
final class Preferences: ObservableObject {
    static let historyLimitOptions = [100, 500, 1_000, 5_000]

    @Published var hideDockIcon: Bool {
        didSet {
            defaults.set(hideDockIcon, forKey: Keys.hideDockIcon)
            defaults.synchronize()
            applyDockPolicy()
        }
    }

    @Published var launchAtLogin: Bool {
        didSet {
            defaults.set(launchAtLogin, forKey: Keys.launchAtLogin)
            updateLaunchAtLogin()
        }
    }

    @Published var hotKeyShortcut: GlobalShortcut {
        didSet {
            hotKeyShortcut.save(to: defaults)
        }
    }

    @Published var pasteDirectly: Bool {
        didSet {
            defaults.set(pasteDirectly, forKey: Keys.pasteDirectly)
        }
    }

    @Published var historyLimit: Int {
        didSet {
            defaults.set(historyLimit, forKey: Keys.historyLimit)
        }
    }

    @Published var capturePaused: Bool {
        didSet {
            defaults.set(capturePaused, forKey: Keys.capturePaused)
        }
    }

    @Published var ignoredApps: [SourceApp] {
        didSet {
            defaults.set(try? JSONEncoder().encode(ignoredApps), forKey: Keys.ignoredApps)
        }
    }

    @Published private(set) var loginItemError: String?

    /// Another app can own the shortcut. Then the system refuses the registration.
    @Published var hotKeyIsUnavailable = false

    private let defaults = UserDefaults.standard

    init() {
        defaults.register(defaults: [
            Keys.pasteDirectly: true,
            Keys.historyLimit: 500
        ])

        hideDockIcon = defaults.bool(forKey: Keys.hideDockIcon)

        if defaults.object(forKey: Keys.launchAtLogin) == nil {
            launchAtLogin = LoginItemController.isEnabled
        } else {
            launchAtLogin = defaults.bool(forKey: Keys.launchAtLogin)
        }

        hotKeyShortcut = GlobalShortcut(defaults: defaults)
        pasteDirectly = defaults.bool(forKey: Keys.pasteDirectly)
        capturePaused = defaults.bool(forKey: Keys.capturePaused)

        let storedLimit = defaults.integer(forKey: Keys.historyLimit)
        historyLimit = Self.historyLimitOptions.contains(storedLimit) ? storedLimit : 500

        ignoredApps = defaults.data(forKey: Keys.ignoredApps)
            .flatMap { try? JSONDecoder().decode([SourceApp].self, from: $0) } ?? []
    }

    func applyDockPolicy() {
        NSApp.setActivationPolicy(hideDockIcon ? .accessory : .regular)

        DispatchQueue.main.async {
            NSApp.setActivationPolicy(self.hideDockIcon ? .accessory : .regular)
        }
    }

    private func updateLaunchAtLogin() {
        do {
            try LoginItemController.setEnabled(launchAtLogin)
            loginItemError = nil
        } catch {
            let actual = LoginItemController.isEnabled
            if launchAtLogin != actual {
                launchAtLogin = actual
            }
            // The revert above runs this method again and clears the error, so set it last.
            loginItemError = error.localizedDescription
        }
    }

    private enum Keys {
        static let hideDockIcon = "hideDockIcon"
        static let launchAtLogin = "launchAtLogin"
        static let pasteDirectly = "pasteDirectly"
        static let historyLimit = "historyLimit"
        static let capturePaused = "capturePaused"
        static let ignoredApps = "ignoredApps"
    }
}
