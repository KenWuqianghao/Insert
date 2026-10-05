import SwiftUI

private enum TrayField {
    case search
    case name
}

struct TrayView: View {
    @ObservedObject var model: PanelModel
    @ObservedObject var library: ClipLibrary
    @ObservedObject var preferences: Preferences

    @FocusState private var focus: TrayField?

    var body: some View {
        VStack(spacing: 0) {
            if model.mode == .previewing, let clip = model.primaryClip {
                ClipPreview(clip: clip, library: library)
                    .id(clip.id)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    .transition(.opacity)
            }

            TopBar(model: model, library: library, preferences: preferences, focus: $focus)
                .frame(height: TrayMetrics.topBarHeight)

            strip
                .frame(height: TrayMetrics.stripHeight)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottom)
        .overlay {
            if case .naming(let target) = model.mode {
                NamePrompt(model: model, target: target, focus: $focus)
                    .transition(.opacity)
            }
        }
        .overlay(alignment: .top) {
            UnevenRoundedRectangle(
                topLeadingRadius: TrayMetrics.cornerRadius,
                topTrailingRadius: TrayMetrics.cornerRadius,
                style: .continuous
            )
            .strokeBorder(Color.white.opacity(0.14), lineWidth: 1)
            .allowsHitTesting(false)
        }
        .ignoresSafeArea()
        .animation(TrayMetrics.animation, value: model.mode)
        .onChange(of: model.showCount) { _, _ in
            focusSearch()
        }
        .onChange(of: model.mode) { _, mode in
            if case .naming = mode {
                focus = .name
            } else {
                focusSearch()
            }
        }
        .onAppear(perform: focusSearch)
    }

    private func focusSearch() {
        // The panel becomes key in the same run loop turn. The focus request must come after that.
        DispatchQueue.main.async {
            focus = .search
        }
    }

    @ViewBuilder
    private var strip: some View {
        if let emptyState = model.emptyState {
            EmptyStateView(state: emptyState)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .padding(.bottom, 16)
                .transition(.opacity)
        } else {
            ScrollViewReader { proxy in
                ScrollView(.horizontal, showsIndicators: false) {
                    LazyHStack(spacing: 16) {
                        ForEach(Array(model.visibleClips.enumerated()), id: \.element.id) { index, clip in
                            card(for: clip, at: index)
                        }
                    }
                    .padding(.horizontal, 22)
                    .padding(.top, 8)
                    .padding(.bottom, 30)
                    .animation(TrayMetrics.animation, value: model.visibleClips.map(\.id))
                }
                .onChange(of: model.selection.primary) { _, id in
                    guard let id else { return }
                    withAnimation(TrayMetrics.animation) {
                        proxy.scrollTo(id)
                    }
                }
            }
        }
    }

    private func card(for clip: Clip, at index: Int) -> some View {
        ClipCard(
            clip: clip,
            thumbnail: clip.kind == .image ? library.thumbnail(for: clip.id) : nil,
            pinColor: library.pinboards.first { $0.id == clip.pinboardID }?.color.color,
            isSelected: model.selection.contains(clip.id),
            isPrimary: model.selection.primary == clip.id,
            isDeleting: model.deletingIDs.contains(clip.id),
            badge: model.isCommandHeld && index < 9 ? index + 1 : nil
        )
        .id(clip.id)
        .transition(.asymmetric(
            insertion: .move(edge: .leading).combined(with: .opacity),
            removal: .scale(scale: 0.9).combined(with: .opacity)
        ))
        .onTapGesture {
            model.click(clip.id, modifiers: NSEvent.modifierFlags)
        }
        .simultaneousGesture(TapGesture(count: 2).onEnded {
            if NSEvent.modifierFlags.intersection([.command, .shift]).isEmpty {
                model.activate(clip, as: .paste)
            }
        })
        .contextMenu {
            ClipMenu(model: model, library: library, clip: clip)
        }
    }
}

private struct ClipMenu: View {
    let model: PanelModel
    let library: ClipLibrary
    let clip: Clip

    var body: some View {
        let targets = model.targets(for: clip)

        Button("Paste") { model.activate(clip, as: .paste) }
        Button("Paste as Plain Text") { model.activate(clip, as: .pastePlainText) }
        Button("Copy") { model.activate(clip, as: .copy) }

        Divider()

        Menu("Pin to") {
            ForEach(library.pinboards) { pinboard in
                Button(pinboard.name) { library.setPinboard(pinboard.id, for: targets) }
            }
            if !library.pinboards.isEmpty {
                Divider()
            }
            Button("New Pinboard…") { model.beginNaming(.newPinboard(pinning: targets)) }
        }

        if clip.pinboardID != nil {
            Button("Unpin") { library.setPinboard(nil, for: targets) }
        }

        Button("Rename…") { model.beginNaming(.clip(clip.id)) }
        Button("Preview") { model.preview(clip) }

        Divider()

        Button("Delete", role: .destructive) { model.delete(targets) }
    }
}

private struct TopBar: View {
    @ObservedObject var model: PanelModel
    @ObservedObject var library: ClipLibrary
    @ObservedObject var preferences: Preferences
    let focus: FocusState<TrayField?>.Binding

    var body: some View {
        HStack(spacing: 14) {
            searchBox

            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 4) {
                    ScopeTab(title: "Clipboard", color: nil, isSelected: model.scope == .history) {
                        model.scope = .history
                    }

                    ForEach(library.pinboards) { pinboard in
                        ScopeTab(
                            title: pinboard.name,
                            color: pinboard.color.color,
                            isSelected: model.scope == .pinboard(pinboard.id)
                        ) {
                            model.scope = .pinboard(pinboard.id)
                        }
                        .contextMenu {
                            pinboardMenu(for: pinboard)
                        }
                    }

                    Button {
                        model.beginNaming(.newPinboard(pinning: []))
                    } label: {
                        Image(systemName: "plus")
                            .font(.system(size: 12, weight: .bold))
                            .frame(width: 26, height: 26)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .foregroundStyle(.secondary)
                    .help("New pinboard")
                }
            }

            Spacer(minLength: 0)

            if preferences.capturePaused {
                NoticePill(symbol: "pause.circle.fill", text: "Capture is paused", actionTitle: "Resume") {
                    preferences.capturePaused = false
                }
            }

            if model.needsPastePermission {
                NoticePill(
                    symbol: "exclamationmark.triangle.fill",
                    text: "Copy only. Allow Accessibility to paste directly.",
                    actionTitle: "Allow…",
                    action: model.actions.requestPastePermission
                )
            }

            kindFilterMenu
            moreMenu
        }
        .padding(.horizontal, 22)
        .padding(.top, 6)
    }

    private var searchBox: some View {
        HStack(spacing: 8) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(.secondary)

            TextField("Search", text: $model.query)
                .textFieldStyle(.plain)
                .font(.system(size: 14))
                .focused(focus, equals: .search)

            if !model.query.isEmpty {
                Button {
                    model.query = ""
                } label: {
                    Image(systemName: "xmark.circle.fill")
                }
                .buttonStyle(.plain)
                .foregroundStyle(.secondary)
            }
        }
        .padding(.horizontal, 10)
        .frame(width: 240, height: 30)
        .background(Color.primary.opacity(0.08), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
    }

    @ViewBuilder
    private func pinboardMenu(for pinboard: Pinboard) -> some View {
        Button("Rename…") { model.beginNaming(.pinboard(pinboard.id)) }

        Menu("Color") {
            ForEach(PinboardColor.allCases) { color in
                Button {
                    library.setColor(color, forPinboard: pinboard.id)
                } label: {
                    if color == pinboard.color {
                        Label(color.name, systemImage: "checkmark")
                    } else {
                        Text(color.name)
                    }
                }
            }
        }

        Divider()

        Button("Delete Pinboard", role: .destructive) { library.deletePinboard(pinboard.id) }
    }

    private var kindFilterMenu: some View {
        Menu {
            Picker("Type", selection: $model.kindFilter) {
                Text("All Types").tag(ClipKind?.none)
                Divider()
                ForEach(ClipKind.allCases) { kind in
                    Label(kind.name, systemImage: kind.symbolName).tag(ClipKind?.some(kind))
                }
            }
            .pickerStyle(.inline)
            .labelsHidden()
        } label: {
            HStack(spacing: 5) {
                Image(systemName: "line.3.horizontal.decrease")
                    .font(.system(size: 13, weight: .semibold))
                if let kind = model.kindFilter {
                    Text(kind.name)
                        .font(.system(size: 12, weight: .semibold))
                }
            }
            .padding(.horizontal, 8)
            .frame(height: 28)
            .foregroundStyle(model.kindFilter == nil ? AnyShapeStyle(.secondary) : AnyShapeStyle(Color.accentColor))
            .contentShape(Rectangle())
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .menuIndicator(.hidden)
        .fixedSize()
        .help("Filter by type")
    }

    private var moreMenu: some View {
        Menu {
            Button("Settings…", action: model.actions.showSettings)
            Button(preferences.capturePaused ? "Resume Capture" : "Pause Capture") {
                preferences.capturePaused.toggle()
            }
            Button("Clear History…", action: model.actions.clearHistory)
            Divider()
            Button("Quit Insert") { NSApp.terminate(nil) }
        } label: {
            Image(systemName: "ellipsis")
                .font(.system(size: 15, weight: .semibold))
                .frame(width: 30, height: 28)
                .foregroundStyle(.secondary)
                .contentShape(Rectangle())
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .menuIndicator(.hidden)
        .fixedSize()
    }
}

private struct ScopeTab: View {
    let title: String
    let color: Color?
    let isSelected: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 6) {
                if let color {
                    Circle()
                        .fill(color)
                        .frame(width: 8, height: 8)
                }
                Text(title)
                    .font(.system(size: 13, weight: isSelected ? .semibold : .medium))
                    .lineLimit(1)
            }
            .padding(.horizontal, 11)
            .frame(height: 28)
            .foregroundStyle(isSelected ? .primary : .secondary)
            .background(Color.primary.opacity(isSelected ? 0.12 : 0), in: Capsule())
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
    }
}

private struct NoticePill: View {
    let symbol: String
    let text: String
    let actionTitle: String
    let action: () -> Void

    var body: some View {
        HStack(spacing: 7) {
            Image(systemName: symbol)
                .foregroundStyle(.orange)
            Text(text)
                .lineLimit(1)
            Button(actionTitle, action: action)
                .buttonStyle(.bordered)
                .controlSize(.small)
        }
        .font(.system(size: 12, weight: .medium))
        .padding(.leading, 10)
        .padding(.trailing, 4)
        .frame(height: 28)
        .background(Color.orange.opacity(0.14), in: Capsule())
        .fixedSize()
    }
}

private struct EmptyStateView: View {
    let state: PanelModel.EmptyState

    private var symbol: String {
        switch state {
        case .noHistory: return "doc.on.clipboard"
        case .noMatches: return "doc.text.magnifyingglass"
        case .emptyPinboard: return "pin"
        }
    }

    private var title: String {
        switch state {
        case .noHistory: return "No Clips"
        case .noMatches: return "No Matches"
        case .emptyPinboard: return "Empty Pinboard"
        }
    }

    private var detail: String {
        switch state {
        case .noHistory: return "Copy something. It shows here."
        case .noMatches: return "Change the search text or the type filter."
        case .emptyPinboard: return "Right-click a clip and select Pin to."
        }
    }

    var body: some View {
        VStack(spacing: 8) {
            Image(systemName: symbol)
                .font(.system(size: 34))
                .foregroundStyle(.tertiary)
            Text(title)
                .font(.system(size: 17, weight: .semibold))
                .foregroundStyle(.secondary)
            Text(detail)
                .font(.system(size: 13))
                .foregroundStyle(.tertiary)
        }
    }
}

private struct NamePrompt: View {
    let model: PanelModel
    let target: PanelModel.NameTarget
    let focus: FocusState<TrayField?>.Binding

    @State private var name = ""

    private var title: String {
        switch target {
        case .clip: return "Rename Clip"
        case .pinboard: return "Rename Pinboard"
        case .newPinboard: return "New Pinboard"
        }
    }

    var body: some View {
        ZStack {
            Color.black.opacity(0.28)

            VStack(alignment: .leading, spacing: 10) {
                Text(title)
                    .font(.system(size: 15, weight: .semibold))

                TextField("Name", text: $name)
                    .textFieldStyle(.roundedBorder)
                    .font(.system(size: 14))
                    .focused(focus, equals: .name)
                    .onSubmit {
                        model.commitName(name)
                    }

                Text("Press Return to save. Press Esc to cancel.")
                    .font(.system(size: 11))
                    .foregroundStyle(.secondary)
            }
            .padding(18)
            .frame(width: 320)
            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
            .shadow(color: .black.opacity(0.3), radius: 18, y: 6)
        }
        .onAppear {
            name = model.currentName(for: target)
        }
    }
}
