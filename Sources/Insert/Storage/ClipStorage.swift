import Foundation

struct LibraryIndex: Codable {
    var clips: [Clip] = []
    var pinboards: [Pinboard] = []
}

/// Files on disk: `index.json`, `payloads/<clip id>.plist`, `thumbnails/<clip id>.png`.
struct ClipStorage {
    private let root: URL
    private let fileManager = FileManager.default

    init(bundleID: String) {
        let applicationSupport = fileManager.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? fileManager.homeDirectoryForCurrentUser.appendingPathComponent("Library/Application Support")
        root = applicationSupport.appendingPathComponent(bundleID, isDirectory: true)
    }

    private var indexURL: URL { root.appendingPathComponent("index.json") }
    private var payloadsURL: URL { root.appendingPathComponent("payloads", isDirectory: true) }
    private var thumbnailsURL: URL { root.appendingPathComponent("thumbnails", isDirectory: true) }

    private func payloadURL(_ id: Clip.ID) -> URL {
        payloadsURL.appendingPathComponent("\(id.uuidString).plist")
    }

    func thumbnailURL(_ id: Clip.ID) -> URL {
        thumbnailsURL.appendingPathComponent("\(id.uuidString).png")
    }

    func loadIndex() -> LibraryIndex {
        guard let data = try? Data(contentsOf: indexURL) else { return LibraryIndex() }
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        return (try? decoder.decode(LibraryIndex.self, from: data)) ?? LibraryIndex()
    }

    func saveIndex(_ index: LibraryIndex) throws {
        try fileManager.createDirectory(at: root, withIntermediateDirectories: true)
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        try encoder.encode(index).write(to: indexURL, options: .atomic)
    }

    func write(_ capture: CapturedClip) throws {
        try fileManager.createDirectory(at: payloadsURL, withIntermediateDirectories: true)
        let encoder = PropertyListEncoder()
        encoder.outputFormat = .binary
        try encoder.encode(capture.payload).write(to: payloadURL(capture.clip.id), options: .atomic)

        if let thumbnail = capture.thumbnail {
            try fileManager.createDirectory(at: thumbnailsURL, withIntermediateDirectories: true)
            try thumbnail.write(to: thumbnailURL(capture.clip.id), options: .atomic)
        }
    }

    func loadPayload(_ id: Clip.ID) -> ClipPayload? {
        guard let data = try? Data(contentsOf: payloadURL(id)) else { return nil }
        return try? PropertyListDecoder().decode(ClipPayload.self, from: data)
    }

    func hasPayload(_ id: Clip.ID) -> Bool {
        fileManager.fileExists(atPath: payloadURL(id).path)
    }

    func removeFiles(for id: Clip.ID) {
        try? fileManager.removeItem(at: payloadURL(id))
        try? fileManager.removeItem(at: thumbnailURL(id))
    }

    /// A crash between a file write and an index write can leave files that no clip owns.
    func removeOrphans(keeping ids: Set<Clip.ID>) {
        for directory in [payloadsURL, thumbnailsURL] {
            let files = (try? fileManager.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)) ?? []
            for file in files {
                let id = UUID(uuidString: file.deletingPathExtension().lastPathComponent)
                if id.map(ids.contains) != true {
                    try? fileManager.removeItem(at: file)
                }
            }
        }
    }
}
