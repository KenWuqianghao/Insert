import Foundation

/// Every method takes the visible clip ids in display order.
struct Selection: Equatable {
    private(set) var ids: Set<Clip.ID> = []
    private(set) var primary: Clip.ID?
    private(set) var anchor: Clip.ID?

    init() {}

    init(only id: Clip.ID?) {
        primary = id
        anchor = id
        ids = id.map { [$0] } ?? []
    }

    func contains(_ id: Clip.ID) -> Bool {
        ids.contains(id)
    }

    mutating func move(by offset: Int, in visible: [Clip.ID]) {
        self = Selection(only: neighbor(at: offset, in: visible))
    }

    mutating func extend(by offset: Int, in visible: [Clip.ID]) {
        guard let target = neighbor(at: offset, in: visible) else {
            self = Selection()
            return
        }
        selectRange(to: target, in: visible)
    }

    mutating func toggle(_ id: Clip.ID, in visible: [Clip.ID]) {
        if ids.contains(id) {
            ids.remove(id)
            if primary == id {
                primary = visible.first(where: ids.contains)
            }
            anchor = primary
        } else {
            ids.insert(id)
            primary = id
            anchor = anchor ?? id
        }
    }

    mutating func selectRange(to id: Clip.ID, in visible: [Clip.ID]) {
        guard
            let targetIndex = visible.firstIndex(of: id),
            let anchorID = anchor ?? primary,
            let anchorIndex = visible.firstIndex(of: anchorID)
        else {
            self = Selection(only: id)
            return
        }

        ids = Set(visible[min(anchorIndex, targetIndex)...max(anchorIndex, targetIndex)])
        primary = id
        anchor = anchorID
    }

    mutating func selectAll(in visible: [Clip.ID]) {
        ids = Set(visible)
        primary = primary.flatMap { ids.contains($0) ? $0 : nil } ?? visible.first
        anchor = visible.first
    }

    mutating func reconcile(with visible: [Clip.ID]) {
        let survivors = visible.filter(ids.contains)
        guard let first = survivors.first else {
            self = Selection(only: visible.first)
            return
        }

        ids = Set(survivors)
        if primary.map(ids.contains) != true {
            primary = first
        }
        if anchor.map(ids.contains) != true {
            anchor = primary
        }
    }

    /// The selection to show after `removed` leaves the list.
    func afterDeleting(_ removed: Set<Clip.ID>, in visible: [Clip.ID]) -> Selection {
        guard let primary, removed.contains(primary) else {
            var kept = self
            kept.ids.subtract(removed)
            if kept.anchor.map(removed.contains) == true {
                kept.anchor = kept.primary
            }
            return kept
        }

        let remaining = visible.filter { !removed.contains($0) }
        guard !remaining.isEmpty else { return Selection() }

        let firstRemovedIndex = visible.firstIndex(where: removed.contains) ?? 0
        return Selection(only: remaining[min(firstRemovedIndex, remaining.count - 1)])
    }

    private func neighbor(at offset: Int, in visible: [Clip.ID]) -> Clip.ID? {
        guard !visible.isEmpty else { return nil }
        let current = primary.flatMap(visible.firstIndex(of:)) ?? 0
        return visible[min(max(current + offset, 0), visible.count - 1)]
    }
}
