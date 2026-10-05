import AppKit

@MainActor
final class ClipLibrary: ObservableObject {
    /// Newest first.
    @Published private(set) var clips: [Clip]
    @Published private(set) var pinboards: [Pinboard]

    var historyLimit: Int {
        didSet {
            update { Self.trim(&$0, to: historyLimit) }
        }
    }

    private let storage: ClipStorage
    private let thumbnails = NSCache<NSUUID, NSImage>()
    private var pendingSave: DispatchWorkItem?

    init(storage: ClipStorage, historyLimit: Int) {
        self.storage = storage
        self.historyLimit = historyLimit

        let index = storage.loadIndex()
        clips = index.clips.filter { storage.hasPayload($0.id) }
        pinboards = index.pinboards
        storage.removeOrphans(keeping: Set(clips.map(\.id)))
    }

    func insert(_ capture: CapturedClip) {
        if let existing = clips.first(where: { $0.contentHash == capture.clip.contentHash }) {
            update { clips in
                Self.moveToFront(existing.id, in: &clips) { $0.source = capture.clip.source ?? $0.source }
            }
            return
        }

        do {
            try storage.write(capture)
        } catch {
            NSLog("Insert could not store a clip: \(error.localizedDescription)")
            return
        }

        update { clips in
            clips.insert(capture.clip, at: 0)
            Self.trim(&clips, to: historyLimit)
        }
    }

    func moveToFront(_ id: Clip.ID) {
        update { Self.moveToFront(id, in: &$0) { _ in } }
    }

    func delete(_ ids: Set<Clip.ID>) {
        update { $0.removeAll { ids.contains($0.id) } }
    }

    func clearHistory() {
        update { $0.removeAll { $0.pinboardID == nil } }
    }

    func setPinboard(_ pinboardID: Pinboard.ID?, for ids: Set<Clip.ID>) {
        update { clips in
            for index in clips.indices where ids.contains(clips[index].id) {
                clips[index].pinboardID = pinboardID
            }
            Self.trim(&clips, to: historyLimit)
        }
    }

    func rename(_ id: Clip.ID, to title: String?) {
        update { clips in
            guard let index = clips.firstIndex(where: { $0.id == id }) else { return }
            clips[index].customTitle = title
        }
    }

    func payload(for id: Clip.ID) -> ClipPayload? {
        storage.loadPayload(id)
    }

    func thumbnail(for id: Clip.ID) -> NSImage? {
        if let cached = thumbnails.object(forKey: id as NSUUID) {
            return cached
        }
        guard let image = NSImage(contentsOf: storage.thumbnailURL(id)) else { return nil }
        thumbnails.setObject(image, forKey: id as NSUUID)
        return image
    }

    @discardableResult
    func addPinboard(named name: String) -> Pinboard.ID {
        let used = Set(pinboards.map(\.color))
        let palette = PinboardColor.allCases
        let color = palette.first { !used.contains($0) } ?? palette[pinboards.count % palette.count]
        let pinboard = Pinboard(id: UUID(), name: name, color: color)
        pinboards.append(pinboard)
        scheduleSave()
        return pinboard.id
    }

    func renamePinboard(_ id: Pinboard.ID, to name: String) {
        updatePinboard(id) { $0.name = name }
    }

    func setColor(_ color: PinboardColor, forPinboard id: Pinboard.ID) {
        updatePinboard(id) { $0.color = color }
    }

    /// The clips of the pinboard go back to the history. The history limit then applies to them.
    func deletePinboard(_ id: Pinboard.ID) {
        pinboards.removeAll { $0.id == id }
        update { clips in
            for index in clips.indices where clips[index].pinboardID == id {
                clips[index].pinboardID = nil
            }
            Self.trim(&clips, to: historyLimit)
        }
        scheduleSave()
    }

    func flush() {
        guard pendingSave != nil else { return }
        pendingSave?.cancel()
        pendingSave = nil
        do {
            try storage.saveIndex(LibraryIndex(clips: clips, pinboards: pinboards))
        } catch {
            NSLog("Insert could not save the clip index: \(error.localizedDescription)")
        }
    }

    private func update(_ change: (inout [Clip]) -> Void) {
        var next = clips
        change(&next)
        guard next != clips else { return }

        let kept = Set(next.map(\.id))
        for clip in clips where !kept.contains(clip.id) {
            storage.removeFiles(for: clip.id)
            thumbnails.removeObject(forKey: clip.id as NSUUID)
        }

        clips = next
        scheduleSave()
    }

    private func updatePinboard(_ id: Pinboard.ID, _ change: (inout Pinboard) -> Void) {
        guard let index = pinboards.firstIndex(where: { $0.id == id }) else { return }
        change(&pinboards[index])
        scheduleSave()
    }

    private func scheduleSave() {
        pendingSave?.cancel()
        let work = DispatchWorkItem { [weak self] in
            self?.flush()
        }
        pendingSave = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.25, execute: work)
    }

    private static func moveToFront(_ id: Clip.ID, in clips: inout [Clip], adjust: (inout Clip) -> Void) {
        guard let index = clips.firstIndex(where: { $0.id == id }) else { return }
        var clip = clips.remove(at: index)
        clip.createdAt = Date()
        adjust(&clip)
        clips.insert(clip, at: 0)
    }

    private static func trim(_ clips: inout [Clip], to limit: Int) {
        var unpinned = 0
        clips.removeAll { clip in
            guard clip.pinboardID == nil else { return false }
            unpinned += 1
            return unpinned > limit
        }
    }
}
