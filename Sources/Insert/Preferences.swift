import AppKit
import Foundation

@MainActor
final class Preferences: ObservableObject {
    @Published var hideDockIcon: Bool {
        didSet {
            UserDefaults.standard.set(hideDockIcon, forKey: Keys.hideDockIcon)
            UserDefaults.standard.synchronize()
            applyDockPolicy()
        }
    }

    @Published var launchAtLogin: Bool {
        didSet {
            UserDefaults.standard.set(launchAtLogin, forKey: Keys.launchAtLogin)
            updateLaunchAtLogin()
        }
    }

    @Published var hotKeyShortcut: GlobalShortcut {
        didSet {
            hotKeyShortcut.save(to: .standard)
        }
    }

    @Published private(set) var loginItemError: String?

    init() {
        hideDockIcon = UserDefaults.standard.bool(forKey: Keys.hideDockIcon)

        if UserDefaults.standard.object(forKey: Keys.launchAtLogin) == nil {
            launchAtLogin = LoginItemController.isEnabled
        } else {
            launchAtLogin = UserDefaults.standard.bool(forKey: Keys.launchAtLogin)
        }

        hotKeyShortcut = GlobalShortcut(defaults: .standard)
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
            loginItemError = error.localizedDescription
            launchAtLogin = LoginItemController.isEnabled
            UserDefaults.standard.set(launchAtLogin, forKey: Keys.launchAtLogin)
        }
    }

    private enum Keys {
        static let hideDockIcon = "hideDockIcon"
        static let launchAtLogin = "launchAtLogin"
    }
}
