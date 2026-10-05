import AppKit
import CryptoKit
import ImageIO
import UniformTypeIdentifiers

/// The raw pasteboard representations of one clip. Only this file reads the type strings.
struct ClipPayload: Codable {
    struct Representation: Codable {
        let type: String
        let data: Data
    }

    let items: [[Representation]]

    var byteCount: Int {
        items.joined().reduce(0) { $0 + $1.data.count }
    }

    var contentHash: String {
        var hasher = SHA256()
        for item in items {
            hasher.update(data: Data([0x1E]))
            for representation in item where !PasteboardType.volatile.contains(representation.type) {
                hasher.update(data: Data(representation.type.utf8))
                withUnsafeBytes(of: UInt64(representation.data.count).littleEndian) { hasher.update(bufferPointer: $0) }
                hasher.update(data: representation.data)
            }
        }
        return hasher.finalize().map { String(format: "%02x", $0) }.joined()
    }

    var plainText: String? {
        if let text = string(PasteboardType.utf8Text) ?? string(PasteboardType.utf16Text, encoding: .utf16) {
            return text
        }

        let fileURLs = self.fileURLs
        if !fileURLs.isEmpty {
            return fileURLs.map(\.path).joined(separator: "\n")
        }

        return string(PasteboardType.url)
    }

    var imageData: Data? {
        items.joined().first { PasteboardType.isImage($0.type) }?.data
    }

    fileprivate var fileURLs: [URL] {
        items.joined()
            .filter { $0.type == PasteboardType.fileURL }
            .compactMap { String(data: $0.data, encoding: .utf8) }
            .compactMap(URL.init(string:))
    }

    fileprivate func data(_ type: String) -> Data? {
        items.joined().first { $0.type == type }?.data
    }

    fileprivate func string(_ type: String, encoding: String.Encoding = .utf8) -> String? {
        data(type).flatMap { String(data: $0, encoding: encoding) }
    }

    fileprivate func hasType(where predicate: (String) -> Bool) -> Bool {
        items.joined().contains { predicate($0.type) }
    }
}

struct CapturedClip {
    let clip: Clip
    let payload: ClipPayload
    let thumbnail: Data?
}

enum PasteboardReader {
    static let maxByteCount = 50 * 1024 * 1024

    static func read(_ pasteboard: NSPasteboard, source: SourceApp?) -> CapturedClip? {
        guard !isPrivate(pasteboard), let payload = payload(from: pasteboard) else { return nil }
        guard let (content, thumbnail) = classify(payload, pasteboard: pasteboard) else { return nil }

        let clip = Clip(
            id: UUID(),
            createdAt: Date(),
            source: source,
            content: content,
            customTitle: nil,
            pinboardID: nil,
            contentHash: payload.contentHash,
            byteCount: payload.byteCount
        )
        return CapturedClip(clip: clip, payload: payload, thumbnail: thumbnail)
    }

    private static func isPrivate(_ pasteboard: NSPasteboard) -> Bool {
        let declared = (pasteboard.types ?? []) + (pasteboard.pasteboardItems ?? []).flatMap(\.types)
        return declared.contains { PasteboardType.privacyMarkers.contains($0.rawValue) }
    }

    private static func payload(from pasteboard: NSPasteboard) -> ClipPayload? {
        var total = 0
        var items: [[ClipPayload.Representation]] = []

        for pasteboardItem in pasteboard.pasteboardItems ?? [] {
            var representations: [ClipPayload.Representation] = []
            for type in pasteboardItem.types where PasteboardType.isStored(type.rawValue) {
                guard let data = pasteboardItem.data(forType: type), !data.isEmpty else { continue }
                total += data.count
                guard total <= maxByteCount else { return nil }
                representations.append(.init(type: type.rawValue, data: data))
            }
            if !representations.isEmpty {
                items.append(representations)
            }
        }

        return items.isEmpty ? nil : ClipPayload(items: items)
    }

    private static func classify(_ payload: ClipPayload, pasteboard: NSPasteboard) -> (ClipContent, Data?)? {
        let fileURLs = payload.fileURLs
        if !fileURLs.isEmpty {
            return (.files(fileURLs), nil)
        }

        if let imageData = payload.imageData, let image = Thumbnail(imageData: imageData) {
            return (.image(image.pixelSize), image.pngData)
        }

        if let url = payload.string(PasteboardType.url).flatMap(URL.init(string:)) {
            return (.link(url, title: payload.string(PasteboardType.urlName)), nil)
        }

        if payload.data(PasteboardType.pdf) != nil {
            return (.pdf, nil)
        }

        if payload.data(PasteboardType.color) != nil, let color = NSColor(from: pasteboard)?.usingColorSpace(.sRGB) {
            let value = ColorValue(
                red: color.redComponent,
                green: color.greenComponent,
                blue: color.blueComponent,
                alpha: color.alphaComponent
            )
            return (.color(value), nil)
        }

        let text = payload.string(PasteboardType.utf8Text) ?? payload.string(PasteboardType.utf16Text, encoding: .utf16)
        let hasText = text.map { !$0.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty } ?? false

        if payload.hasType(where: PasteboardType.isRichText) {
            let plain = hasText ? text : richTextString(in: payload)
            return plain.map { (.richText(TextSnippet($0)), nil) }
        }

        if hasText, let text {
            let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
            if let url = webURL(from: trimmed) {
                return (.link(url, title: nil), nil)
            }
            if let color = ColorValue(hex: trimmed) {
                return (.color(color), nil)
            }
            return (.text(TextSnippet(text)), nil)
        }

        if let html = payload.string(PasteboardType.html) {
            let stripped = html
                .replacingOccurrences(of: "<[^>]+>", with: " ", options: .regularExpression)
                .replacingOccurrences(of: "\\s+", with: " ", options: .regularExpression)
                .trimmingCharacters(in: .whitespacesAndNewlines)
            return stripped.isEmpty ? nil : (.richText(TextSnippet(stripped)), nil)
        }

        if payload.hasType(where: PasteboardType.isMedia) {
            return (.media, nil)
        }

        // Whitespace-only text reaches this point. It is not worth a card.
        return text == nil ? (.data, nil) : nil
    }

    private static func richTextString(in payload: ClipPayload) -> String? {
        let attributed = payload.data(PasteboardType.rtf).flatMap { NSAttributedString(rtf: $0, documentAttributes: nil) }
            ?? payload.data(PasteboardType.rtfd).flatMap { NSAttributedString(rtfd: $0, documentAttributes: nil) }
        guard let string = attributed?.string, !string.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return nil
        }
        return string
    }

    private static func webURL(from text: String) -> URL? {
        guard
            !text.contains(where: \.isWhitespace),
            let url = URL(string: text),
            let scheme = url.scheme?.lowercased(),
            scheme == "http" || scheme == "https",
            url.host?.isEmpty == false
        else {
            return nil
        }
        return url
    }
}

enum PasteboardWriter {
    static func write(_ payload: ClipPayload, to pasteboard: NSPasteboard) {
        let items = payload.items.map { representations in
            let item = NSPasteboardItem()
            for representation in representations {
                item.setData(representation.data, forType: NSPasteboard.PasteboardType(representation.type))
            }
            return item
        }

        pasteboard.clearContents()
        pasteboard.writeObjects(items)
    }

    static func write(plainText: String, to pasteboard: NSPasteboard) {
        pasteboard.clearContents()
        pasteboard.setString(plainText, forType: .string)
    }
}

private enum PasteboardType {
    static let utf8Text = NSPasteboard.PasteboardType.string.rawValue
    static let utf16Text = "public.utf16-plain-text"
    static let html = NSPasteboard.PasteboardType.html.rawValue
    static let rtf = NSPasteboard.PasteboardType.rtf.rawValue
    static let rtfd = NSPasteboard.PasteboardType.rtfd.rawValue
    static let url = NSPasteboard.PasteboardType.URL.rawValue
    static let urlName = "public.url-name"
    static let fileURL = NSPasteboard.PasteboardType.fileURL.rawValue
    static let pdf = NSPasteboard.PasteboardType.pdf.rawValue
    static let color = NSPasteboard.PasteboardType.color.rawValue

    static let privacyMarkers: Set<String> = [
        "org.nspasteboard.ConcealedType",
        "org.nspasteboard.TransientType",
        "org.nspasteboard.AutoGeneratedType"
    ]

    /// These carry timestamps or session ids, so two copies of the same content differ.
    static let volatile: Set<String> = [
        "com.apple.webarchive",
        "org.chromium.web-custom-data"
    ]

    private static let stored: Set<String> = [
        utf8Text, utf16Text, html, rtf, rtfd, url, urlName, fileURL, pdf, color,
        NSPasteboard.PasteboardType.tabularText.rawValue,
        NSPasteboard.PasteboardType.png.rawValue,
        NSPasteboard.PasteboardType.tiff.rawValue,
        "public.jpeg",
        "public.heic",
        "public.heif",
        "com.compuserve.gif",
        "public.svg-image",
        "public.mpeg-4",
        "com.apple.quicktime-movie"
    ]

    private static let mediaPrefixes = ["public.movie", "public.audio", "public.video"]
    private static let mediaTypes: Set<String> = ["public.mpeg-4", "com.apple.quicktime-movie"]
    private static let imageTypes: Set<String> = [
        NSPasteboard.PasteboardType.png.rawValue,
        NSPasteboard.PasteboardType.tiff.rawValue,
        "public.jpeg",
        "public.heic",
        "public.heif",
        "com.compuserve.gif"
    ]

    static func isStored(_ type: String) -> Bool {
        stored.contains(type) || volatile.contains(type) || isImage(type) || isMedia(type)
    }

    static func isImage(_ type: String) -> Bool {
        imageTypes.contains(type) || type.hasPrefix("public.image")
    }

    static func isMedia(_ type: String) -> Bool {
        mediaTypes.contains(type) || mediaPrefixes.contains(where: type.hasPrefix)
    }

    static func isRichText(_ type: String) -> Bool {
        type == rtf || type == rtfd
    }
}

private struct Thumbnail {
    static let maxPixelSize = 560

    let pixelSize: PixelSize
    let pngData: Data

    init?(imageData: Data) {
        guard
            let source = CGImageSourceCreateWithData(imageData as CFData, nil),
            let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
            let width = properties[kCGImagePropertyPixelWidth] as? Int,
            let height = properties[kCGImagePropertyPixelHeight] as? Int,
            width > 0, height > 0
        else {
            return nil
        }

        let options: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceThumbnailMaxPixelSize: Self.maxPixelSize
        ]
        let output = NSMutableData()
        guard
            let thumbnail = CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary),
            let destination = CGImageDestinationCreateWithData(output, UTType.png.identifier as CFString, 1, nil)
        else {
            return nil
        }

        CGImageDestinationAddImage(destination, thumbnail, nil)
        guard CGImageDestinationFinalize(destination) else { return nil }

        pixelSize = PixelSize(width: width, height: height)
        pngData = output as Data
    }
}
