import AppKit
import Combine

@MainActor
final class PanelModel: ObservableObject {
    enum Scope: Hashable {
        case history
        case pinboard(Pinboard.ID)
    }

    enum NameTarget: Hashable {
        case clip(Clip.ID)
        case pinboard(Pinboard.ID)
        case newPinboard(pinning: Set<Clip.ID>)
    }

    /// The preview always shows the primary clip of the selection, so it holds no clip id.
    enum Mode: Equatable {
        case browsing
        case previewing
        case naming(NameTarget)
    }

    enum EmptyState {
        case noHistory
        case noMatches
        case emptyPinboard
    }

    struct Actions {
        var hide: () -> Void = {}
        var activate: (Clip, PasteAction) -> Void = { _, _ in }
        var showSettings: () -> Void = {}
        var clearHistory: () -> Void = {}
        var requestPastePermission: () -> Void = {}
    }

    let library: ClipLibrary
    var actions = Actions()

    @Published var scope: Scope = .history {
        didSet { if scope != oldValue { refresh(resettingSelection: true) } }
    }

    @Published var query = "" {
        didSet { if query != oldValue { refresh(resettingSelection: true) } }
    }

    @Published var kindFilter: ClipKind? {
        didSet { if kindFilter != oldValue { refresh(resettingSelection: true) } }
    }

    @Published private(set) var visibleClips: [Clip] = []
    @Published private(set) var selection = Selection()
    @Published private(set) var mode: Mode = .browsing
    @Published private(set) var deletingIDs: Set<Clip.ID> = []
    @Published private(set) var showCount = 0
    @Published private(set) var needsPastePermission = false
    @Published var isCommandHeld = false

    private var cancellables = Set<AnyCancellable>()

    init(library: ClipLibrary) {
        self.library = library

        library.$clips
            .sink { [weak self] clips in
                self?.refresh(clips: clips, resettingSelection: false)
            }
            .store(in: &cancellables)

        library.$pinboards
            .sink { [weak self] pinboards in
                guard let self, case .pinboard(let id) = self.scope else { return }
                if !pinboards.contains(where: { $0.id == id }) {
                    self.scope = .history
                }
            }
            .store(in: &cancellables)
    }

    var primaryClip: Clip? {
        selection.primary.flatMap { id in visibleClips.first { $0.id == id } }
    }

    var emptyState: EmptyState? {
        guard visibleClips.isEmpty else { return nil }
        if !query.isEmpty || kindFilter != nil {
            return .noMatches
        }
        return scope == .history ? .noHistory : .emptyPinboard
    }

    func prepareToShow(needsPastePermission: Bool) {
        self.needsPastePermission = needsPastePermission
        mode = .browsing
        kindFilter = nil
        query = ""
        refresh(resettingSelection: true)
        showCount += 1
    }

    func didHide() {
        mode = .browsing
        isCommandHeld = false
    }

    func perform(_ command: PanelCommand) {
        switch command {
        case .move(let offset):
            selection.move(by: offset, in: visibleIDs)
        case .extend(let offset):
            selection.extend(by: offset, in: visibleIDs)
        case .selectAll:
            selection.selectAll(in: visibleIDs)
        case .activate(let action):
            if let clip = primaryClip {
                activate(clip, as: action)
            }
        case .activateIndex(let index):
            if visibleClips.indices.contains(index) {
                activate(visibleClips[index], as: .paste)
            }
        case .deleteSelection:
            delete(selection.ids)
        case .togglePreview:
            mode = mode == .browsing && primaryClip != nil ? .previewing : .browsing
        case .cancel:
            cancel()
        case .switchScope(let offset):
            switchScope(by: offset)
        case .openSettings:
            actions.showSettings()
        case .hide:
            actions.hide()
        }
    }

    func click(_ id: Clip.ID, modifiers: NSEvent.ModifierFlags) {
        if modifiers.contains(.shift) {
            selection.selectRange(to: id, in: visibleIDs)
        } else if modifiers.contains(.command) {
            selection.toggle(id, in: visibleIDs)
        } else {
            selection = Selection(only: id)
        }
    }

    func activate(_ clip: Clip, as action: PasteAction) {
        actions.activate(clip, action)
    }

    func preview(_ clip: Clip) {
        selection = Selection(only: clip.id)
        mode = .previewing
    }

    /// A menu on a selected card acts on the whole selection. A menu on another card acts on that card only.
    func targets(for clip: Clip) -> Set<Clip.ID> {
        selection.contains(clip.id) ? selection.ids : [clip.id]
    }

    func delete(_ ids: Set<Clip.ID>) {
        guard deletingIDs.isEmpty, !ids.isEmpty else { return }

        selection = selection.afterDeleting(ids, in: visibleIDs)
        deletingIDs = ids

        // The cards fade out first. Then the library removes them.
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.16) { [weak self] in
            self?.library.delete(ids)
            self?.deletingIDs = []
        }
    }

    func beginNaming(_ target: NameTarget) {
        mode = .naming(target)
    }

    func currentName(for target: NameTarget) -> String {
        switch target {
        case .clip(let id):
            return library.clips.first { $0.id == id }?.customTitle ?? ""
        case .pinboard(let id):
            return library.pinboards.first { $0.id == id }?.name ?? ""
        case .newPinboard:
            return ""
        }
    }

    func commitName(_ text: String) {
        guard case .naming(let target) = mode else { return }
        let name = text.trimmingCharacters(in: .whitespacesAndNewlines)
        mode = .browsing

        switch target {
        case .clip(let id):
            library.rename(id, to: name.isEmpty ? nil : name)
        case .pinboard(let id):
            if !name.isEmpty {
                library.renamePinboard(id, to: name)
            }
        case .newPinboard(let clipIDs):
            guard !name.isEmpty else { return }
            let id = library.addPinboard(named: name)
            if clipIDs.isEmpty {
                scope = .pinboard(id)
            } else {
                library.setPinboard(id, for: clipIDs)
            }
        }
    }

    private var visibleIDs: [Clip.ID] {
        visibleClips.map(\.id)
    }

    private func cancel() {
        if mode != .browsing {
            mode = .browsing
        } else if !query.isEmpty {
            query = ""
        } else {
            actions.hide()
        }
    }

    private func switchScope(by offset: Int) {
        let scopes = [Scope.history] + library.pinboards.map { Scope.pinboard($0.id) }
        let current = scopes.firstIndex(of: scope) ?? 0
        scope = scopes[(current + offset + scopes.count) % scopes.count]
    }

    private func refresh(clips: [Clip]? = nil, resettingSelection: Bool) {
        let trimmedQuery = query.trimmingCharacters(in: .whitespacesAndNewlines)
        let visible = (clips ?? library.clips).filter { clip in
            if case .pinboard(let id) = scope, clip.pinboardID != id { return false }
            if let kindFilter, clip.kind != kindFilter { return false }
            return trimmedQuery.isEmpty || clip.matches(trimmedQuery)
        }

        if visible != visibleClips {
            visibleClips = visible
        }

        let ids = visible.map(\.id)
        if resettingSelection {
            selection = Selection(only: ids.first)
        } else {
            selection.reconcile(with: ids)
        }

        if mode == .previewing, selection.primary == nil {
            mode = .browsing
        }
    }
}
