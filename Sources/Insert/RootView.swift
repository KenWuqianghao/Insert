import SwiftUI

struct RootView: View {
    @ObservedObject var clipboardStore: ClipboardStore
    @ObservedObject var preferences: Preferences

    let onHide: () -> Void
    let onSelect: (ClipboardItem) -> Void

    @State private var searchText = ""
    @State private var selectedItemID: ClipboardItem.ID?
    @State private var selectedItemIDs: Set<ClipboardItem.ID> = []
    @State private var selectionAnchorItemID: ClipboardItem.ID?
    @State private var deletingItemIDs: Set<ClipboardItem.ID> = []
    @State private var isRecordingShortcut = false
    @FocusState private var searchFocused: Bool

    private var filteredItems: [ClipboardItem] {
        let query = searchText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !query.isEmpty else { return clipboardStore.items }
        return clipboardStore.items.filter {
            $0.searchText.localizedCaseInsensitiveContains(query)
                || $0.title.localizedCaseInsensitiveContains(query)
                || $0.preview.localizedCaseInsensitiveContains(query)
                || $0.kind.rawValue.localizedCaseInsensitiveContains(query)
                || $0.sourceApp.localizedCaseInsensitiveContains(query)
        }
    }

    var body: some View {
        ZStack {
            VStack(spacing: 14) {
                header

                if filteredItems.isEmpty {
                    EmptyClipboardView(hasSearch: !searchText.isEmpty)
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                        .transition(.scale(scale: 0.98).combined(with: .opacity))
                } else {
                    ScrollViewReader { proxy in
                        ScrollView(.horizontal, showsIndicators: false) {
                            LazyHStack(spacing: 12) {
                                ForEach(filteredItems) { item in
                                    ClipboardCard(
                                        item: item,
                                        isSelected: selectedItemIDs.contains(item.id),
                                        isPrimarySelection: item.id == selectedItemID,
                                        isDeleting: deletingItemIDs.contains(item.id)
                                    )
                                        .id(item.id)
                                        .transition(.asymmetric(
                                            insertion: .move(edge: .trailing).combined(with: .opacity),
                                            removal: .move(edge: .leading).combined(with: .opacity)
                                        ))
                                        .onTapGesture {
                                            if selectItemFromClick(item) {
                                                onSelect(item)
                                            }
                                        }
                                }
                            }
                            .padding(.horizontal, 22)
                            .padding(.bottom, 20)
                            .animation(cardAnimation, value: filteredItems.map(\.id))
                            .animation(cardAnimation, value: selectedItemID)
                            .animation(cardAnimation, value: selectedItemIDs)
                            .animation(cardAnimation, value: deletingItemIDs)
                        }
                        .onChange(of: selectedItemID) { _, newValue in
                            guard let newValue else { return }
                            withAnimation(cardAnimation) {
                                proxy.scrollTo(newValue, anchor: .center)
                            }
                        }
                    }
                }
            }

            if isRecordingShortcut {
                ShortcutRecorderOverlay(currentShortcut: preferences.hotKeyShortcut.displayName)
                    .transition(.scale(scale: 0.96).combined(with: .opacity))
            }
        }
        .animation(cardAnimation, value: filteredItems.isEmpty)
        .animation(cardAnimation, value: isRecordingShortcut)
        .padding(.top, 18)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(PanelBackground())
        .onReceive(NotificationCenter.default.publisher(for: .insertPanelWillShow)) { _ in
            selectMostRecentItem()
            searchFocused = true
        }
        .onReceive(NotificationCenter.default.publisher(for: .insertKeyCommand)) { notification in
            guard let command = notification.object as? InsertKeyCommand else { return }
            handleKeyCommand(command)
        }
        .onReceive(NotificationCenter.default.publisher(for: .insertRecordedShortcut)) { notification in
            guard let shortcut = notification.object as? GlobalShortcut else { return }
            preferences.hotKeyShortcut = shortcut
            setShortcutRecording(false)
            searchFocused = true
        }
        .onReceive(NotificationCenter.default.publisher(for: .insertShortcutRecordingCancelled)) { _ in
            setShortcutRecording(false)
            searchFocused = true
        }
        .onChange(of: searchText) { _, _ in
            selectMostRecentItem()
        }
        .onChange(of: clipboardStore.items) { _, _ in
            keepSelectionValid()
        }
    }

    private var header: some View {
        HStack(spacing: 12) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: 16, weight: .semibold))
                .foregroundStyle(.secondary)

            TextField("Search", text: $searchText)
                .textFieldStyle(.plain)
                .font(.system(size: 17, weight: .medium))
                .focused($searchFocused)

            if !searchText.isEmpty {
                Button {
                    searchText = ""
                } label: {
                    Image(systemName: "xmark.circle.fill")
                }
                .buttonStyle(.plain)
                .foregroundStyle(.secondary)
            }

            Divider()
                .frame(height: 24)

            Button {
                setShortcutRecording(true)
            } label: {
                Image(systemName: "keyboard")
                    .font(.system(size: 16, weight: .semibold))
            }
            .buttonStyle(.plain)
            .frame(width: 28, height: 28)
            .help("Change keyboard shortcut")

            Menu {
                Toggle("Hide Dock Icon", isOn: $preferences.hideDockIcon)
                Toggle("Open at Login", isOn: $preferences.launchAtLogin)

                Divider()

                Text("Shortcut: \(preferences.hotKeyShortcut.displayName)")

                Button("Change Shortcut...") {
                    setShortcutRecording(true)
                }

                if let loginItemError = preferences.loginItemError {
                    Divider()
                    Text(loginItemError)
                }

                Divider()

                Button("Quit Insert") {
                    NSApp.terminate(nil)
                }
            } label: {
                Image(systemName: "gearshape")
                    .font(.system(size: 17, weight: .semibold))
            }
            .menuStyle(.button)
            .buttonStyle(.plain)
            .frame(width: 28, height: 28)

            Button {
                onHide()
            } label: {
                Image(systemName: "chevron.down")
                    .font(.system(size: 17, weight: .semibold))
            }
            .buttonStyle(.plain)
            .frame(width: 28, height: 28)
        }
        .padding(.horizontal, 18)
        .frame(height: 46)
        .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .stroke(.white.opacity(0.14), lineWidth: 1)
        )
        .padding(.horizontal, 22)
    }

    private func selectMostRecentItem() {
        withAnimation(cardAnimation) {
            selectOnly(filteredItems.first?.id)
        }
    }

    private func keepSelectionValid() {
        guard !filteredItems.isEmpty else {
            withAnimation(cardAnimation) {
                clearSelection()
            }
            return
        }

        selectedItemIDs = Set(selectedItemIDs.filter { id in
            filteredItems.contains { $0.id == id }
        })

        if let selectedItemID, filteredItems.contains(where: { $0.id == selectedItemID }) {
            if selectedItemIDs.isEmpty {
                selectedItemIDs = [selectedItemID]
            }
            return
        }

        selectMostRecentItem()
    }

    private func handleKeyCommand(_ command: InsertKeyCommand) {
        guard !isRecordingShortcut else { return }
        guard !filteredItems.isEmpty else { return }

        switch command {
        case .movePrevious:
            moveSelection(by: -1)
        case .moveNext:
            moveSelection(by: 1)
        case .extendPrevious:
            extendSelection(by: -1)
        case .extendNext:
            extendSelection(by: 1)
        case .selectAll:
            selectAllItems()
        case .copySelection:
            copySelectedItem()
        case .deleteSelection:
            deleteSelectedItem()
        }
    }

    private func moveSelection(by offset: Int) {
        guard !filteredItems.isEmpty else { return }

        let currentIndex = selectedItemID.flatMap { id in
            filteredItems.firstIndex { $0.id == id }
        } ?? 0

        let nextIndex = min(max(currentIndex + offset, 0), filteredItems.count - 1)
        withAnimation(cardAnimation) {
            selectOnly(filteredItems[nextIndex].id)
        }
    }

    private func extendSelection(by offset: Int) {
        guard !filteredItems.isEmpty else { return }

        let currentIndex = selectedItemID.flatMap { id in
            filteredItems.firstIndex { $0.id == id }
        } ?? 0

        let nextIndex = min(max(currentIndex + offset, 0), filteredItems.count - 1)
        withAnimation(cardAnimation) {
            selectRange(to: filteredItems[nextIndex].id)
        }
    }

    private func selectAllItems() {
        withAnimation(cardAnimation) {
            selectedItemIDs = Set(filteredItems.map(\.id))
            selectedItemID = filteredItems.first?.id
            selectionAnchorItemID = filteredItems.first?.id
        }
    }

    private func copySelectedItem() {
        guard
            let selectedItemID,
            let selectedItem = filteredItems.first(where: { $0.id == selectedItemID })
        else {
            return
        }

        onSelect(selectedItem)
    }

    private func deleteSelectedItem() {
        guard deletingItemIDs.isEmpty else { return }
        guard
            let selectedItemID,
            let selectedIndex = filteredItems.firstIndex(where: { $0.id == selectedItemID })
        else {
            return
        }

        let selectedIDs = selectedItemIDs.isEmpty ? [selectedItemID] : selectedItemIDs
        let selectedItems = filteredItems.filter { selectedIDs.contains($0.id) }
        guard !selectedItems.isEmpty else { return }

        let selectedIndexes = selectedItems.compactMap { item in
            filteredItems.firstIndex { $0.id == item.id }
        }
        let nextSelectionIndex = selectedIndexes.min() ?? selectedIndex
        let remainingItems = filteredItems.filter { !selectedIDs.contains($0.id) }

        withAnimation(cardAnimation) {
            deletingItemIDs = selectedIDs

            if remainingItems.isEmpty {
                clearSelection()
            } else {
                let nextIndex = min(nextSelectionIndex, remainingItems.count - 1)
                selectOnly(remainingItems[nextIndex].id)
            }
        }

        DispatchQueue.main.asyncAfter(deadline: .now() + 0.16) {
            withAnimation(cardAnimation) {
                clipboardStore.delete(selectedItems)
                deletingItemIDs = []
            }
        }
    }

    private func selectItemFromClick(_ item: ClipboardItem) -> Bool {
        let modifiers = NSEvent.modifierFlags.intersection(.deviceIndependentFlagsMask)

        withAnimation(cardAnimation) {
            if modifiers.contains(.shift) {
                selectRange(to: item.id)
            } else if modifiers.contains(.command) {
                toggleSelection(of: item.id)
            } else {
                selectOnly(item.id)
            }
        }

        return !modifiers.contains(.shift) && !modifiers.contains(.command)
    }

    private func selectOnly(_ itemID: ClipboardItem.ID?) {
        selectedItemID = itemID
        selectionAnchorItemID = itemID
        if let itemID {
            selectedItemIDs = [itemID]
        } else {
            selectedItemIDs = []
        }
    }

    private func clearSelection() {
        selectedItemID = nil
        selectionAnchorItemID = nil
        selectedItemIDs = []
    }

    private func toggleSelection(of itemID: ClipboardItem.ID) {
        if selectedItemIDs.contains(itemID) {
            selectedItemIDs.remove(itemID)
            if selectedItemID == itemID {
                selectedItemID = filteredItems.first { selectedItemIDs.contains($0.id) }?.id
            }
            selectionAnchorItemID = selectedItemID
        } else {
            selectedItemIDs.insert(itemID)
            selectedItemID = itemID
            selectionAnchorItemID = selectionAnchorItemID ?? itemID
        }
    }

    private func selectRange(to itemID: ClipboardItem.ID) {
        guard
            let targetIndex = filteredItems.firstIndex(where: { $0.id == itemID })
        else {
            selectOnly(itemID)
            return
        }

        let anchorID = selectionAnchorItemID ?? selectedItemID ?? itemID
        guard let anchorIndex = filteredItems.firstIndex(where: { $0.id == anchorID }) else {
            selectOnly(itemID)
            return
        }

        let bounds = min(anchorIndex, targetIndex)...max(anchorIndex, targetIndex)
        selectedItemIDs = Set(filteredItems[bounds].map(\.id))
        selectedItemID = itemID
        selectionAnchorItemID = anchorID
    }

    private func setShortcutRecording(_ isRecording: Bool) {
        isRecordingShortcut = isRecording
        searchFocused = !isRecording
        NotificationCenter.default.post(name: .insertShortcutRecordingChanged, object: isRecording)
    }
}

private struct ClipboardCard: View {
    let item: ClipboardItem
    let isSelected: Bool
    let isPrimarySelection: Bool
    let isDeleting: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text(item.sourceApp)
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)

                Spacer(minLength: 8)

                TimelineView(.periodic(from: .now, by: 60)) { context in
                    Text(item.createdAt.coarseRelativeDescription(now: context.date))
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(.tertiary)
                        .lineLimit(1)
                }
            }

            Text(item.title)
                .font(.system(size: 17, weight: .semibold))
                .lineLimit(item.thumbnailImage == nil ? 2 : 1)
                .foregroundStyle(.primary)

            if let image = item.thumbnailImage {
                Image(nsImage: image)
                    .resizable()
                    .scaledToFill()
                    .frame(maxWidth: .infinity, maxHeight: 86)
                    .clipShape(RoundedRectangle(cornerRadius: 7, style: .continuous))
                    .overlay(
                        RoundedRectangle(cornerRadius: 7, style: .continuous)
                            .stroke(.white.opacity(0.14), lineWidth: 1)
                    )
            } else {
                HStack(alignment: .top, spacing: 9) {
                    Image(systemName: item.kind.symbolName)
                        .font(.system(size: 20, weight: .semibold))
                        .foregroundStyle(.secondary)
                        .frame(width: 26)

                    Text(item.preview)
                        .font(.system(size: 13))
                        .lineSpacing(3)
                        .foregroundStyle(.secondary)
                        .lineLimit(5)
                }
            }

            Spacer(minLength: 0)
        }
        .padding(14)
        .frame(width: 232, height: 178, alignment: .topLeading)
        .background(cardBackground)
        .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .stroke(isSelected ? Color.accentColor.opacity(0.92) : .white.opacity(0.16), lineWidth: isPrimarySelection ? 3 : (isSelected ? 2 : 1))
        )
        .shadow(color: isSelected ? Color.accentColor.opacity(0.34) : .clear, radius: 14, y: 0)
        .scaleEffect(isDeleting ? 0.94 : (isSelected ? 1.02 : 1))
        .offset(x: isDeleting ? -26 : 0, y: isDeleting ? 8 : 0)
        .opacity(isDeleting ? 0 : 1)
        .contentShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
        .animation(cardAnimation, value: isSelected)
        .animation(cardAnimation, value: isPrimarySelection)
        .animation(cardAnimation, value: isDeleting)
        .allowsHitTesting(!isDeleting)
    }

    private var cardBackground: some ShapeStyle {
        LinearGradient(
            colors: [
                Color(nsColor: .windowBackgroundColor).opacity(0.92),
                Color(nsColor: .controlBackgroundColor).opacity(0.86)
            ],
            startPoint: .topLeading,
            endPoint: .bottomTrailing
        )
    }
}

private var cardAnimation: Animation {
    .spring(response: 0.24, dampingFraction: 0.84, blendDuration: 0.08)
}

private extension Date {
    func coarseRelativeDescription(now: Date) -> String {
        let seconds = max(0, Int(now.timeIntervalSince(self)))

        if seconds < 60 {
            return "Just now"
        }

        let minutes = seconds / 60
        if minutes < 60 {
            return "\(minutes) \(minutes == 1 ? "min" : "min")"
        }

        let hours = minutes / 60
        if hours < 24 {
            return "\(hours) \(hours == 1 ? "hr" : "hr")"
        }

        let days = hours / 24
        if days < 7 {
            return "\(days) \(days == 1 ? "day" : "days")"
        }

        let weeks = days / 7
        if weeks < 5 {
            return "\(weeks) \(weeks == 1 ? "week" : "weeks")"
        }

        let months = days / 30
        if months < 12 {
            return "\(months) \(months == 1 ? "mo" : "mo")"
        }

        let years = days / 365
        return "\(years) \(years == 1 ? "yr" : "yr")"
    }
}

private struct EmptyClipboardView: View {
    let hasSearch: Bool

    var body: some View {
        VStack(spacing: 10) {
            Image(systemName: hasSearch ? "doc.text.magnifyingglass" : "doc.on.clipboard")
                .font(.system(size: 34, weight: .regular))
                .foregroundStyle(.tertiary)

            Text(hasSearch ? "No Matches" : "Clipboard Empty")
                .font(.system(size: 17, weight: .semibold))
                .foregroundStyle(.secondary)
        }
    }
}

private struct ShortcutRecorderOverlay: View {
    let currentShortcut: String

    var body: some View {
        VStack(spacing: 14) {
            Image(systemName: "keyboard")
                .font(.system(size: 28, weight: .semibold))
                .foregroundStyle(.primary)

            VStack(spacing: 5) {
                Text("Press New Shortcut")
                    .font(.system(size: 18, weight: .semibold))

                Text("Current: \(currentShortcut)")
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(.secondary)
            }

            Text("Use at least one modifier. Esc cancels.")
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(.tertiary)
        }
        .padding(.horizontal, 28)
        .padding(.vertical, 22)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .stroke(.white.opacity(0.18), lineWidth: 1)
        )
        .shadow(color: .black.opacity(0.32), radius: 20, y: 8)
    }
}

private struct PanelBackground: View {
    var body: some View {
        RoundedRectangle(cornerRadius: 18, style: .continuous)
            .fill(.ultraThinMaterial)
            .overlay(
                LinearGradient(
                    colors: [
                        Color.white.opacity(0.20),
                        Color.white.opacity(0.04),
                        Color.black.opacity(0.12)
                    ],
                    startPoint: .top,
                    endPoint: .bottom
                )
                .clipShape(RoundedRectangle(cornerRadius: 18, style: .continuous))
            )
            .overlay(
                RoundedRectangle(cornerRadius: 18, style: .continuous)
                    .stroke(.white.opacity(0.20), lineWidth: 1)
            )
            .shadow(color: .black.opacity(0.34), radius: 28, y: 10)
    }
}
