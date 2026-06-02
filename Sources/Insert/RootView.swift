import SwiftUI

struct RootView: View {
    @ObservedObject var clipboardStore: ClipboardStore
    @ObservedObject var preferences: Preferences

    let onHide: () -> Void
    let onSelect: (ClipboardItem) -> Void

    @State private var searchText = ""
    @State private var selectedItemID: ClipboardItem.ID?
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
                } else {
                    ScrollViewReader { proxy in
                        ScrollView(.horizontal, showsIndicators: false) {
                            LazyHStack(spacing: 12) {
                                ForEach(filteredItems) { item in
                                    ClipboardCard(item: item, isSelected: item.id == selectedItemID)
                                        .id(item.id)
                                        .onTapGesture {
                                            selectedItemID = item.id
                                            onSelect(item)
                                        }
                                }
                            }
                            .padding(.horizontal, 22)
                            .padding(.bottom, 20)
                        }
                        .onChange(of: selectedItemID) { _, newValue in
                            guard let newValue else { return }
                            withAnimation(.easeOut(duration: 0.16)) {
                                proxy.scrollTo(newValue, anchor: .center)
                            }
                        }
                    }
                }
            }

            if isRecordingShortcut {
                ShortcutRecorderOverlay(currentShortcut: preferences.hotKeyShortcut.displayName)
            }
        }
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
        selectedItemID = filteredItems.first?.id
    }

    private func keepSelectionValid() {
        guard !filteredItems.isEmpty else {
            selectedItemID = nil
            return
        }

        if let selectedItemID, filteredItems.contains(where: { $0.id == selectedItemID }) {
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
        selectedItemID = filteredItems[nextIndex].id
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
        guard
            let selectedItemID,
            let selectedIndex = filteredItems.firstIndex(where: { $0.id == selectedItemID })
        else {
            return
        }

        let selectedItem = filteredItems[selectedIndex]
        let remainingItems = filteredItems.filter { $0.id != selectedItem.id }

        if remainingItems.isEmpty {
            self.selectedItemID = nil
        } else {
            let nextIndex = min(selectedIndex, remainingItems.count - 1)
            self.selectedItemID = remainingItems[nextIndex].id
        }

        clipboardStore.delete(selectedItem)
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

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text(item.sourceApp)
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)

                Spacer(minLength: 8)

                Text(item.createdAt, style: .relative)
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(.tertiary)
                    .lineLimit(1)
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
                .stroke(isSelected ? Color.accentColor.opacity(0.92) : .white.opacity(0.16), lineWidth: isSelected ? 3 : 1)
        )
        .shadow(color: isSelected ? Color.accentColor.opacity(0.36) : .clear, radius: 14, y: 0)
        .contentShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
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
