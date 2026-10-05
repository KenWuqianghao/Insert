import Foundation

struct SourceApp: Codable, Hashable {
    let bundleID: String
    let name: String
}

struct PixelSize: Codable, Hashable {
    let width: Int
    let height: Int
}

struct ColorValue: Codable, Hashable {
    let red: Double
    let green: Double
    let blue: Double
    let alpha: Double

    init(red: Double, green: Double, blue: Double, alpha: Double) {
        self.red = red
        self.green = green
        self.blue = blue
        self.alpha = alpha
    }

    init?(hex text: String) {
        guard text.hasPrefix("#") else { return nil }
        var digits = String(text.dropFirst())
        guard digits.allSatisfy(\.isHexDigit) else { return nil }

        if digits.count == 3 {
            digits = digits.map { "\($0)\($0)" }.joined()
        }
        guard digits.count == 6 || digits.count == 8, let value = UInt64(digits, radix: 16) else { return nil }

        let rgba = digits.count == 8 ? value : (value << 8) | 0xFF
        red = Double((rgba >> 24) & 0xFF) / 255
        green = Double((rgba >> 16) & 0xFF) / 255
        blue = Double((rgba >> 8) & 0xFF) / 255
        alpha = Double(rgba & 0xFF) / 255
    }

    var hex: String {
        let channels = [red, green, blue].map { Int(($0 * 255).rounded()) }
        let rgb = String(format: "#%02X%02X%02X", channels[0], channels[1], channels[2])
        return alpha < 0.999 ? rgb + String(format: "%02X", Int((alpha * 255).rounded())) : rgb
    }

    var isLight: Bool {
        0.299 * red + 0.587 * green + 0.114 * blue > 0.62
    }
}

/// The index keeps only the start of a long text. The payload file keeps all of it.
struct TextSnippet: Codable, Hashable {
    static let limit = 8_000

    let text: String
    let characterCount: Int

    init(_ fullText: String) {
        text = String(fullText.prefix(Self.limit))
        characterCount = fullText.count
    }

    var firstLine: String? {
        text.split(whereSeparator: \.isNewline)
            .lazy
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .first { !$0.isEmpty }
    }
}

enum ClipKind: String, CaseIterable, Identifiable {
    case text
    case richText
    case link
    case image
    case files
    case color
    case pdf
    case media
    case data

    var id: String { rawValue }

    var name: String {
        switch self {
        case .text: return "Text"
        case .richText: return "Rich Text"
        case .link: return "Link"
        case .image: return "Image"
        case .files: return "Files"
        case .color: return "Color"
        case .pdf: return "PDF"
        case .media: return "Media"
        case .data: return "Data"
        }
    }

    var symbolName: String {
        switch self {
        case .text: return "text.alignleft"
        case .richText: return "textformat"
        case .link: return "link"
        case .image: return "photo"
        case .files: return "doc"
        case .color: return "paintpalette"
        case .pdf: return "doc.richtext"
        case .media: return "play.rectangle"
        case .data: return "shippingbox"
        }
    }
}

enum ClipContent: Codable, Hashable {
    case text(TextSnippet)
    case richText(TextSnippet)
    case link(URL, title: String?)
    case image(PixelSize)
    case files([URL])
    case color(ColorValue)
    case pdf
    case media
    case data

    var kind: ClipKind {
        switch self {
        case .text: return .text
        case .richText: return .richText
        case .link: return .link
        case .image: return .image
        case .files: return .files
        case .color: return .color
        case .pdf: return .pdf
        case .media: return .media
        case .data: return .data
        }
    }

    var displayTitle: String {
        switch self {
        case .text(let snippet), .richText(let snippet):
            return snippet.firstLine ?? kind.name
        case .link(let url, let title):
            return title ?? url.host ?? url.absoluteString
        case .files(let urls):
            guard urls.count == 1, let url = urls.first else { return "\(urls.count) Files" }
            return url.lastPathComponent
        case .color(let color):
            return color.hex
        case .image:
            return "Image"
        case .pdf:
            return "PDF Document"
        case .media:
            return "Media"
        case .data:
            return "Clipboard Data"
        }
    }

    var searchText: String {
        switch self {
        case .text(let snippet), .richText(let snippet):
            return snippet.text
        case .link(let url, let title):
            return [title, url.absoluteString].compactMap { $0 }.joined(separator: " ")
        case .files(let urls):
            return urls.map(\.path).joined(separator: " ")
        case .color(let color):
            return color.hex
        case .image, .pdf, .media, .data:
            return kind.name
        }
    }

    func footer(byteCount: Int) -> String {
        switch self {
        case .text(let snippet), .richText(let snippet):
            return Self.counted(snippet.characterCount, "character")
        case .link(let url, _):
            return Self.counted(url.absoluteString.count, "character")
        case .image(let size):
            return "\(size.width.formatted()) × \(size.height.formatted())"
        case .files(let urls):
            return Self.counted(urls.count, "file")
        case .color(let color):
            let channels = [color.red, color.green, color.blue].map { String(Int(($0 * 255).rounded())) }
            return "RGB " + channels.joined(separator: ", ")
        case .pdf, .media, .data:
            return ByteCountFormatter.string(fromByteCount: Int64(byteCount), countStyle: .file)
        }
    }

    private static func counted(_ count: Int, _ noun: String) -> String {
        "\(count.formatted()) \(noun)\(count == 1 ? "" : "s")"
    }
}

struct Clip: Codable, Hashable, Identifiable {
    let id: UUID
    var createdAt: Date
    var source: SourceApp?
    let content: ClipContent
    var customTitle: String?
    var pinboardID: Pinboard.ID?
    let contentHash: String
    let byteCount: Int

    var kind: ClipKind { content.kind }
    var footerText: String { content.footer(byteCount: byteCount) }

    func matches(_ query: String) -> Bool {
        [content.searchText, customTitle, source?.name, kind.name]
            .compactMap { $0 }
            .contains { $0.localizedCaseInsensitiveContains(query) }
    }
}

enum PinboardColor: String, Codable, CaseIterable, Identifiable {
    case red
    case orange
    case yellow
    case green
    case teal
    case blue
    case purple
    case pink

    var id: String { rawValue }
    var name: String { rawValue.capitalized }
}

struct Pinboard: Codable, Hashable, Identifiable {
    let id: UUID
    var name: String
    var color: PinboardColor
}
