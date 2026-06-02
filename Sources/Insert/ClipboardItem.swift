import AppKit
import Foundation

struct ClipboardItem: Identifiable, Equatable, Codable {
    let id: UUID
    let createdAt: Date
    let sourceApp: String
    let kind: ClipboardKind
    let titleText: String
    let previewText: String
    let searchText: String
    let pasteboardItems: [StoredPasteboardItem]

    init(
        id: UUID = UUID(),
        createdAt: Date = Date(),
        sourceApp: String = "Clipboard",
        kind: ClipboardKind,
        titleText: String,
        previewText: String,
        searchText: String,
        pasteboardItems: [StoredPasteboardItem]
    ) {
        self.id = id
        self.createdAt = createdAt
        self.sourceApp = sourceApp
        self.kind = kind
        self.titleText = titleText
        self.previewText = previewText
        self.searchText = searchText
        self.pasteboardItems = pasteboardItems
    }

    init(id: UUID = UUID(), content: String, createdAt: Date = Date(), sourceApp: String = "Clipboard") {
        let data = Data(content.utf8)
        self.init(
            id: id,
            createdAt: createdAt,
            sourceApp: sourceApp,
            kind: .text,
            titleText: Self.title(forText: content),
            previewText: content.trimmingCharacters(in: .whitespacesAndNewlines),
            searchText: content,
            pasteboardItems: [
                StoredPasteboardItem(representations: [
                    StoredPasteboardRepresentation(typeName: NSPasteboard.PasteboardType.string.rawValue, data: data)
                ])
            ]
        )
    }

    var title: String {
        titleText
    }

    var preview: String {
        previewText
    }

    var content: String {
        searchText
    }

    var thumbnailImage: NSImage? {
        for item in pasteboardItems {
            for representation in item.representations {
                let type = NSPasteboard.PasteboardType(representation.typeName)
                if type == .tiff || type == .png || type.rawValue.hasPrefix("public.image"),
                   let image = NSImage(data: representation.data) {
                    return image
                }
            }
        }

        return nil
    }

    var fingerprint: String {
        let typeNames = pasteboardItems
            .flatMap(\.representations)
            .map(\.typeName)
            .joined(separator: "|")
        let payloadSize = pasteboardItems
            .flatMap(\.representations)
            .map(\.data.count)
            .reduce(0, +)

        return "\(kind.rawValue)|\(titleText)|\(previewText)|\(typeNames)|\(payloadSize)"
    }

    static func title(forText text: String) -> String {
        let firstLine = text
            .split(whereSeparator: \.isNewline)
            .first
            .map(String.init) ?? text

        let trimmed = firstLine.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? "Text" : trimmed
    }
}

enum ClipboardKind: String, Codable {
    case text = "Text"
    case image = "Image"
    case file = "File"
    case url = "URL"
    case richText = "Rich Text"
    case html = "HTML"
    case pdf = "PDF"
    case color = "Color"
    case media = "Media"
    case data = "Data"

    var symbolName: String {
        switch self {
        case .text:
            return "text.alignleft"
        case .image:
            return "photo"
        case .file:
            return "doc"
        case .url:
            return "link"
        case .richText:
            return "textformat"
        case .html:
            return "chevron.left.forwardslash.chevron.right"
        case .pdf:
            return "doc.richtext"
        case .color:
            return "paintpalette"
        case .media:
            return "play.rectangle"
        case .data:
            return "shippingbox"
        }
    }
}

struct StoredPasteboardItem: Equatable, Codable {
    let representations: [StoredPasteboardRepresentation]
}

struct StoredPasteboardRepresentation: Equatable, Codable {
    let typeName: String
    let data: Data
}
