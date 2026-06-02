import AppKit
import Foundation

@MainActor
final class ClipboardStore: ObservableObject {
    @Published private(set) var items: [ClipboardItem] = [] {
        didSet {
            saveItems()
        }
    }

    private var changeCount = NSPasteboard.general.changeCount
    private var timer: Timer?
    private let maxItems = 80
    private let storageURL: URL

    init() {
        let applicationSupport = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
        let baseURL = applicationSupport ?? FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Application Support")
        storageURL = baseURL.appendingPathComponent("Insert/ClipboardHistory.json")
        items = loadItems()
    }

    func start() {
        guard timer == nil else { return }
        readCurrentPasteboardIfNeeded(force: true)
        timer = Timer.scheduledTimer(withTimeInterval: 0.55, repeats: true) { [weak self] _ in
            Task { @MainActor in
                self?.readCurrentPasteboardIfNeeded(force: false)
            }
        }
    }

    func stop() {
        timer?.invalidate()
        timer = nil
    }

    func copyToPasteboard(_ item: ClipboardItem) {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()

        let writableItems = item.pasteboardItems.map { storedItem in
            let pasteboardItem = NSPasteboardItem()
            for representation in storedItem.representations {
                pasteboardItem.setData(
                    representation.data,
                    forType: NSPasteboard.PasteboardType(representation.typeName)
                )
            }
            return pasteboardItem
        }

        if !writableItems.isEmpty {
            pasteboard.writeObjects(writableItems)
        }

        changeCount = pasteboard.changeCount
    }

    func delete(_ item: ClipboardItem) {
        items.removeAll { $0.id == item.id }
    }

    private func readCurrentPasteboardIfNeeded(force: Bool) {
        let pasteboard = NSPasteboard.general
        guard force || pasteboard.changeCount != changeCount else { return }
        changeCount = pasteboard.changeCount

        guard let item = ClipboardItem(pasteboard: pasteboard) else { return }
        if items.first?.fingerprint == item.fingerprint { return }

        items.removeAll { $0.fingerprint == item.fingerprint }
        items.insert(item, at: 0)

        if items.count > maxItems {
            items.removeLast(items.count - maxItems)
        }
    }

    private func loadItems() -> [ClipboardItem] {
        do {
            let data = try Data(contentsOf: storageURL)
            let decoder = JSONDecoder()
            decoder.dateDecodingStrategy = .iso8601

            if let currentItems = try? decoder.decode([ClipboardItem].self, from: data) {
                return currentItems
            }

            let legacyItems = try decoder.decode([LegacyClipboardItem].self, from: data)
            return legacyItems.map {
                ClipboardItem(id: $0.id, content: $0.content, createdAt: $0.createdAt, sourceApp: $0.sourceApp)
            }
        } catch {
            return []
        }
    }

    private func saveItems() {
        do {
            try FileManager.default.createDirectory(
                at: storageURL.deletingLastPathComponent(),
                withIntermediateDirectories: true
            )

            let encoder = JSONEncoder()
            encoder.dateEncodingStrategy = .iso8601
            encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
            let data = try encoder.encode(items)
            try data.write(to: storageURL, options: .atomic)
        } catch {
            assertionFailure("Failed to save clipboard history: \(error.localizedDescription)")
        }
    }
}

private struct LegacyClipboardItem: Codable {
    let id: UUID
    let content: String
    let createdAt: Date
    let sourceApp: String
}

private extension ClipboardItem {
    init?(pasteboard: NSPasteboard) {
        let appName = NSWorkspace.shared.frontmostApplication?.localizedName ?? "Clipboard"
        let storedItems = StoredPasteboardItem.items(from: pasteboard)
        guard !storedItems.isEmpty else { return nil }

        let descriptor = ClipboardDescriptor(pasteboard: pasteboard, storedItems: storedItems)
        guard !descriptor.searchText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || descriptor.kind != .text else {
            return nil
        }

        self.init(
            createdAt: Date(),
            sourceApp: appName,
            kind: descriptor.kind,
            titleText: descriptor.title,
            previewText: descriptor.preview,
            searchText: descriptor.searchText,
            pasteboardItems: storedItems
        )
    }
}

private extension StoredPasteboardItem {
    static func items(from pasteboard: NSPasteboard) -> [StoredPasteboardItem] {
        if let pasteboardItems = pasteboard.pasteboardItems, !pasteboardItems.isEmpty {
            return pasteboardItems.compactMap { pasteboardItem in
                let representations = pasteboardItem.types.compactMap { type -> StoredPasteboardRepresentation? in
                    guard
                        StoredPasteboardTypePolicy.shouldStore(type),
                        let data = pasteboardItem.data(forType: type),
                        !data.isEmpty
                    else {
                        return nil
                    }

                    return StoredPasteboardRepresentation(typeName: type.rawValue, data: data)
                }

                return representations.isEmpty ? nil : StoredPasteboardItem(representations: representations)
            }
        }

        let representations = pasteboard.types?.compactMap { type -> StoredPasteboardRepresentation? in
            guard
                StoredPasteboardTypePolicy.shouldStore(type),
                let data = pasteboard.data(forType: type),
                !data.isEmpty
            else {
                return nil
            }

            return StoredPasteboardRepresentation(typeName: type.rawValue, data: data)
        } ?? []

        return representations.isEmpty ? [] : [StoredPasteboardItem(representations: representations)]
    }
}

private enum StoredPasteboardTypePolicy {
    static func shouldStore(_ type: NSPasteboard.PasteboardType) -> Bool {
        let raw = type.rawValue

        if raw.hasPrefix("dyn.") {
            return false
        }

        if raw.contains("promised") || raw.contains("Promise") || raw.contains("transient") {
            return false
        }

        let allowedTypes: Set<String> = [
            NSPasteboard.PasteboardType.string.rawValue,
            NSPasteboard.PasteboardType.html.rawValue,
            NSPasteboard.PasteboardType.rtf.rawValue,
            NSPasteboard.PasteboardType.rtfd.rawValue,
            NSPasteboard.PasteboardType.tabularText.rawValue,
            NSPasteboard.PasteboardType.URL.rawValue,
            NSPasteboard.PasteboardType.fileURL.rawValue,
            NSPasteboard.PasteboardType.png.rawValue,
            NSPasteboard.PasteboardType.tiff.rawValue,
            NSPasteboard.PasteboardType.pdf.rawValue,
            NSPasteboard.PasteboardType.color.rawValue,
            "public.jpeg",
            "public.heic",
            "public.heif",
            "public.image",
            "public.movie",
            "public.audio",
            "public.video",
            "public.mpeg-4",
            "com.apple.quicktime-movie",
            "com.compuserve.gif",
            "public.svg-image",
            "public.url",
            "public.file-url",
            "public.utf8-plain-text",
            "public.utf16-plain-text",
            "com.apple.webarchive",
            "org.chromium.web-custom-data"
        ]

        return allowedTypes.contains(raw)
            || raw.hasPrefix("public.image")
            || raw.hasPrefix("public.movie")
            || raw.hasPrefix("public.audio")
            || raw.hasPrefix("public.video")
    }
}

private struct ClipboardDescriptor {
    let kind: ClipboardKind
    let title: String
    let preview: String
    let searchText: String

    init(pasteboard: NSPasteboard, storedItems: [StoredPasteboardItem]) {
        let allRepresentations = storedItems.flatMap(\.representations)
        let types = allRepresentations.map { NSPasteboard.PasteboardType($0.typeName) }

        if let fileTitle = Self.fileTitle(from: allRepresentations), types.contains(.fileURL) {
            let count = storedItems.count
            kind = .file
            title = count > 1 ? "\(count) Files" : fileTitle
            preview = count > 1 ? fileTitle : "File"
            searchText = "\(title) \(preview)"
            return
        }

        if let urlText = Self.string(for: .URL, in: allRepresentations) ?? Self.string(for: NSPasteboard.PasteboardType("public.url"), in: allRepresentations) {
            kind = .url
            title = URL(string: urlText)?.host ?? ClipboardItem.title(forText: urlText)
            preview = urlText
            searchText = urlText
            return
        }

        if types.contains(.tiff) || types.contains(.png) || types.contains(where: { $0.rawValue.hasPrefix("public.image") }) {
            kind = .image
            title = "Image"
            preview = Self.imagePreview(from: allRepresentations)
            searchText = "\(title) \(preview)"
            return
        }

        if types.contains(.pdf) {
            kind = .pdf
            title = "PDF"
            preview = Self.bytePreview(from: allRepresentations)
            searchText = "\(title) \(preview)"
            return
        }

        if types.contains(.rtf) || types.contains(.rtfd) {
            let plainText = Self.string(for: .string, in: allRepresentations)
            kind = .richText
            title = plainText.map(ClipboardItem.title(forText:)) ?? "Rich Text"
            preview = plainText ?? "Formatted text"
            searchText = plainText ?? title
            return
        }

        if types.contains(.html), let html = Self.string(for: .html, in: allRepresentations) {
            kind = .html
            title = "HTML"
            preview = Self.strippedHTML(html)
            searchText = "\(preview) \(html)"
            return
        }

        if types.contains(.color) {
            kind = .color
            title = "Color"
            preview = "Color sample"
            searchText = title
            return
        }

        if let text = Self.string(for: .string, in: allRepresentations) {
            kind = .text
            title = ClipboardItem.title(forText: text)
            preview = text.trimmingCharacters(in: .whitespacesAndNewlines)
            searchText = text
            return
        }

        if types.contains(where: { $0.rawValue.hasPrefix("public.movie") || $0.rawValue.hasPrefix("public.audio") || $0.rawValue.hasPrefix("public.video") }) {
            kind = .media
            title = "Media"
            preview = Self.bytePreview(from: allRepresentations)
            searchText = "\(title) \(preview)"
            return
        }

        kind = .data
        title = "Clipboard Data"
        preview = Self.bytePreview(from: allRepresentations)
        searchText = "\(title) \(preview)"
    }

    private static func string(for type: NSPasteboard.PasteboardType, in representations: [StoredPasteboardRepresentation]) -> String? {
        guard let data = representations.first(where: { $0.typeName == type.rawValue })?.data else { return nil }
        return String(data: data, encoding: .utf8)
            ?? String(data: data, encoding: .utf16)
            ?? String(data: data, encoding: .ascii)
    }

    private static func fileTitle(from representations: [StoredPasteboardRepresentation]) -> String? {
        let urls = representations
            .filter { $0.typeName == NSPasteboard.PasteboardType.fileURL.rawValue }
            .compactMap { String(data: $0.data, encoding: .utf8) }
            .compactMap(URL.init(string:))

        guard let firstURL = urls.first else { return nil }
        return firstURL.lastPathComponent.isEmpty ? firstURL.path : firstURL.lastPathComponent
    }

    private static func imagePreview(from representations: [StoredPasteboardRepresentation]) -> String {
        for representation in representations {
            if let image = NSImage(data: representation.data), image.size.width > 0, image.size.height > 0 {
                return "\(Int(image.size.width)) x \(Int(image.size.height))"
            }
        }

        return bytePreview(from: representations)
    }

    private static func bytePreview(from representations: [StoredPasteboardRepresentation]) -> String {
        let byteCount = representations.map(\.data.count).reduce(0, +)
        return ByteCountFormatter.string(fromByteCount: Int64(byteCount), countStyle: .file)
    }

    private static func strippedHTML(_ html: String) -> String {
        html.replacingOccurrences(of: "<[^>]+>", with: " ", options: .regularExpression)
            .replacingOccurrences(of: "\\s+", with: " ", options: .regularExpression)
            .trimmingCharacters(in: .whitespacesAndNewlines)
    }
}
