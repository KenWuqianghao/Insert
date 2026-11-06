import CryptoKit
import Foundation

/// A license key that has a valid signature: `INSERT1-<payload>.<signature>`.
/// The payload is the base64url JSON that the site signs with Ed25519. Verification is offline.
struct License: Equatable {
    let key: String
    let email: String
    let order: String

    private static let prefix = "INSERT1-"
    private static let publicKey = try! Curve25519.Signing.PublicKey(
        rawRepresentation: Data(base64Encoded: "gmVV4YARbaaHZ1376PcTuERSmLV7vA1fWbisZT0aAKs=")!
    )

    private struct Payload: Decodable {
        let email: String
        let order: String
    }

    init(key rawKey: String) throws(LicenseError) {
        let key = rawKey.trimmingCharacters(in: .whitespacesAndNewlines)
        guard key.hasPrefix(Self.prefix) else { throw .malformed }

        let parts = key.dropFirst(Self.prefix.count).split(separator: ".", omittingEmptySubsequences: false)
        guard parts.count == 2,
              let payload = Self.base64URLDecoded(parts[0]),
              let signature = Self.base64URLDecoded(parts[1])
        else { throw .malformed }

        guard Self.publicKey.isValidSignature(signature, for: payload) else { throw .invalidSignature }

        guard let fields = try? JSONDecoder().decode(Payload.self, from: payload),
              !fields.email.isEmpty, !fields.order.isEmpty
        else { throw .malformed }

        self.key = key
        self.email = fields.email
        self.order = fields.order
    }

    private static func base64URLDecoded(_ text: Substring) -> Data? {
        guard !text.isEmpty, !text.contains("=") else { return nil }
        var base64 = text.replacingOccurrences(of: "-", with: "+").replacingOccurrences(of: "_", with: "/")
        base64 += String(repeating: "=", count: (4 - base64.count % 4) % 4)
        return Data(base64Encoded: base64)
    }
}

enum LicenseError: Error, Equatable {
    case malformed
    case invalidSignature

    var message: String {
        switch self {
        case .malformed:
            return "This is not an Insert license key. Copy the full key from the email or the purchase page."
        case .invalidSignature:
            return "This license key is not valid. Copy the full key from the email or the purchase page."
        }
    }
}

/// What the user can do with Insert now. The license and the trial start decide it, so nothing stores it.
enum AccessState: Equatable {
    case trial(daysLeft: Int)
    case trialEnded
    case licensed(email: String)

    static let trialLength: TimeInterval = 14 * 24 * 60 * 60

    init(license: License?, trialStart: Date, now: Date) {
        if let license {
            self = .licensed(email: license.email)
            return
        }
        let remaining = trialStart.addingTimeInterval(Self.trialLength).timeIntervalSince(now)
        if remaining > 0 {
            self = .trial(daysLeft: Int((remaining / (24 * 60 * 60)).rounded(.up)))
        } else {
            self = .trialEnded
        }
    }

    var opensTray: Bool {
        if case .trialEnded = self { return false }
        return true
    }
}
