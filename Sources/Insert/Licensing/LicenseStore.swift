import Foundation

@MainActor
final class LicenseStore: ObservableObject {
    static let buyURL = URL(string: "https://insert-app.vercel.app/buy")!

    @Published private(set) var license: License?
    /// The result of the last failed activation, from the License tab or from an `insert://activate` link.
    @Published private(set) var activationError: LicenseError?

    let trialStart: Date

    private let defaults = UserDefaults.standard

    init() {
        if let start = defaults.object(forKey: Keys.trialStart) as? Date {
            trialStart = start
        } else {
            trialStart = Date()
            defaults.set(trialStart, forKey: Keys.trialStart)
        }
        license = defaults.string(forKey: Keys.licenseKey).flatMap { try? License(key: $0) }
    }

    /// The trial counts down while the app runs, so read the state when you need it.
    var state: AccessState {
        AccessState(license: license, trialStart: trialStart, now: Date())
    }

    @discardableResult
    func activate(key: String) -> Bool {
        do {
            let license = try License(key: key)
            defaults.set(license.key, forKey: Keys.licenseKey)
            self.license = license
            activationError = nil
            return true
        } catch {
            activationError = error
            return false
        }
    }

    func deactivate() {
        defaults.removeObject(forKey: Keys.licenseKey)
        license = nil
        activationError = nil
    }

    /// `insert://activate?key=<percent-encoded key>`
    func activate(url: URL) {
        let key = URLComponents(url: url, resolvingAgainstBaseURL: false)?
            .queryItems?.first { $0.name == "key" }?.value
        activate(key: key ?? "")
    }

    private enum Keys {
        static let trialStart = "trialStartDate"
        static let licenseKey = "licenseKey"
    }
}
