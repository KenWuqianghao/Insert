import AppKit
import SwiftUI

enum TrayMetrics {
    static let topBarHeight: CGFloat = 54
    static let cardSize: CGFloat = 232
    static let cardCornerRadius: CGFloat = 14
    static let stripHeight: CGFloat = cardSize + 38
    static let baseHeight = topBarHeight + stripHeight
    static let cornerRadius: CGFloat = 16

    static let animation = Animation.spring(response: 0.24, dampingFraction: 0.84, blendDuration: 0.08)
}

extension Color {
    static let cardBackground = Color(nsColor: NSColor(name: nil) { appearance in
        appearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua ? NSColor(white: 0.17, alpha: 1) : .white
    })
}

extension PinboardColor {
    var color: Color {
        switch self {
        case .red: return .red
        case .orange: return .orange
        case .yellow: return .yellow
        case .green: return .green
        case .teal: return .teal
        case .blue: return .blue
        case .purple: return .purple
        case .pink: return .pink
        }
    }
}

extension ColorValue {
    var color: Color {
        Color(.sRGB, red: red, green: green, blue: blue, opacity: alpha)
    }
}

extension Date {
    func coarseRelativeDescription(now: Date) -> String {
        let seconds = max(0, Int(now.timeIntervalSince(self)))
        if seconds < 60 {
            return "Just now"
        }

        let minutes = seconds / 60
        if minutes < 60 {
            return "\(minutes) min ago"
        }

        let hours = minutes / 60
        if hours < 24 {
            return "\(hours) hr ago"
        }

        let days = hours / 24
        if days < 7 {
            return "\(days) \(days == 1 ? "day" : "days") ago"
        }

        let weeks = days / 7
        if weeks < 5 {
            return "\(weeks) \(weeks == 1 ? "week" : "weeks") ago"
        }

        let months = days / 30
        if months < 12 {
            return "\(months) mo ago"
        }

        return "\(days / 365) yr ago"
    }
}

/// App icons and the header tint that comes from each icon. Each bundle id is resolved one time.
@MainActor
final class AppIconStore {
    struct Entry {
        let icon: NSImage?
        let tint: Color
    }

    static let shared = AppIconStore()

    private static let neutral = Entry(icon: nil, tint: Color(white: 0.42))
    private var entries: [String: Entry] = [:]
    private let fileIcons = NSCache<NSURL, NSImage>()

    func entry(for source: SourceApp?) -> Entry {
        guard let bundleID = source?.bundleID else { return Self.neutral }
        if let entry = entries[bundleID] {
            return entry
        }

        let entry: Entry
        if let url = NSWorkspace.shared.urlForApplication(withBundleIdentifier: bundleID) {
            let icon = NSWorkspace.shared.icon(forFile: url.path)
            entry = Entry(icon: icon, tint: Self.tint(for: icon) ?? Self.neutral.tint)
        } else {
            entry = Self.neutral
        }

        entries[bundleID] = entry
        return entry
    }

    func fileIcon(for url: URL) -> NSImage {
        if let cached = fileIcons.object(forKey: url as NSURL) {
            return cached
        }
        let icon = NSWorkspace.shared.icon(forFile: url.path)
        fileIcons.setObject(icon, forKey: url as NSURL)
        return icon
    }

    /// Finds the strongest hue of the icon, then limits saturation and brightness so white text stays readable.
    private static func tint(for icon: NSImage) -> Color? {
        let side = 24
        var proposedRect = CGRect(x: 0, y: 0, width: side, height: side)
        guard let image = icon.cgImage(forProposedRect: &proposedRect, context: nil, hints: nil) else { return nil }

        var pixels = [UInt8](repeating: 0, count: side * side * 4)
        let drawn = pixels.withUnsafeMutableBytes { buffer -> Bool in
            guard let context = CGContext(
                data: buffer.baseAddress,
                width: side,
                height: side,
                bitsPerComponent: 8,
                bytesPerRow: side * 4,
                space: CGColorSpaceCreateDeviceRGB(),
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
            ) else {
                return false
            }
            context.draw(image, in: CGRect(x: 0, y: 0, width: side, height: side))
            return true
        }
        guard drawn else { return nil }

        struct Bucket {
            var weight = 0.0
            var red = 0.0
            var green = 0.0
            var blue = 0.0
        }

        var buckets = [Bucket](repeating: Bucket(), count: 12)
        var opaqueCount = 0.0
        var colorfulCount = 0.0
        var valueSum = 0.0

        for offset in stride(from: 0, to: pixels.count, by: 4) {
            let alpha = Double(pixels[offset + 3]) / 255
            guard alpha > 0.5 else { continue }

            let red = Double(pixels[offset]) / 255 / alpha
            let green = Double(pixels[offset + 1]) / 255 / alpha
            let blue = Double(pixels[offset + 2]) / 255 / alpha
            let value = max(red, green, blue)
            let chroma = value - min(red, green, blue)
            opaqueCount += 1
            valueSum += value

            guard value > 0.2, chroma / value > 0.3 else { continue }
            colorfulCount += 1

            var hue: Double
            if value == red {
                hue = ((green - blue) / chroma).truncatingRemainder(dividingBy: 6)
            } else if value == green {
                hue = (blue - red) / chroma + 2
            } else {
                hue = (red - green) / chroma + 4
            }
            hue = (hue < 0 ? hue + 6 : hue) / 6

            let index = min(buckets.count - 1, Int(hue * Double(buckets.count)))
            let weight = chroma
            buckets[index].weight += weight
            buckets[index].red += red * weight
            buckets[index].green += green * weight
            buckets[index].blue += blue * weight
        }

        guard opaqueCount > 0 else { return nil }
        guard colorfulCount / opaqueCount > 0.08, let top = buckets.max(by: { $0.weight < $1.weight }), top.weight > 0 else {
            let value = valueSum / opaqueCount
            return Color(white: min(max(value * 0.55, 0.26), 0.46))
        }

        let dominant = NSColor(
            srgbRed: top.red / top.weight,
            green: top.green / top.weight,
            blue: top.blue / top.weight,
            alpha: 1
        )
        let hue = dominant.hueComponent
        let isBrightHue = (0.11...0.5).contains(hue)
        let saturation = min(max(dominant.saturationComponent, 0.5), 0.86)
        let brightness = min(max(dominant.brightnessComponent, 0.48), isBrightHue ? 0.66 : 0.8)
        return Color(hue: hue, saturation: saturation, brightness: brightness)
    }
}
