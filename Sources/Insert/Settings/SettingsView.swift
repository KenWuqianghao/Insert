import AppKit
import ApplicationServices
import Carbon
import SwiftUI
import UniformTypeIdentifiers

enum SettingsPane: String, CaseIterable {
    case general
    case privacy

    var title: String {
        switch self {
        case .general:
            return "General"
        case .privacy:
            return "Privacy"
        }
    }

    var symbolName: String {
        switch self {
        case .general:
            return "gearshape"
        case .privacy:
            return "hand.raised"
        }
    }

    var toolbarIdentifier: NSToolbarItem.Identifier {
        NSToolbarItem.Identifier(rawValue)
    }
}

@MainActor
final class SettingsSelection: ObservableObject {
    @Published var pane = SettingsPane.general
}

@MainActor
final class SettingsWindowController: NSObject, NSToolbarDelegate {
    private let preferences: Preferences
    private let actions: SettingsView.Actions
    private let selection = SettingsSelection()
    private var window: NSWindow?

    init(preferences: Preferences, actions: SettingsView.Actions) {
        self.preferences = preferences
        self.actions = actions
    }

    func show() {
        let window = self.window ?? makeWindow()
        self.window = window

        // This also works when the Dock icon is hidden and the activation policy is accessory.
        NSApp.activate(ignoringOtherApps: true)
        window.makeKeyAndOrderFront(nil)
    }

    private func makeWindow() -> NSWindow {
        // The hosting view sizes the window, so the window follows the height of the selected pane.
        let window = NSWindow(contentRect: .zero, styleMask: [.titled, .closable], backing: .buffered, defer: true)
        window.contentView = NSHostingView(
            rootView: SettingsView(selection: selection, preferences: preferences, actions: actions)
        )
        window.isReleasedWhenClosed = false
        window.toolbarStyle = .preference

        let toolbar = NSToolbar(identifier: "InsertSettings")
        toolbar.delegate = self
        toolbar.displayMode = .iconAndLabel
        toolbar.allowsUserCustomization = false
        window.toolbar = toolbar

        select(selection.pane, in: window)
        window.center()
        return window
    }

    private func select(_ pane: SettingsPane, in window: NSWindow) {
        selection.pane = pane
        window.title = pane.title
        window.toolbar?.selectedItemIdentifier = pane.toolbarIdentifier
    }

    @objc private func selectPane(_ sender: NSToolbarItem) {
        guard let window, let pane = SettingsPane(rawValue: sender.itemIdentifier.rawValue) else { return }
        select(pane, in: window)
    }

    func toolbarAllowedItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] {
        SettingsPane.allCases.map(\.toolbarIdentifier)
    }

    func toolbarDefaultItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] {
        SettingsPane.allCases.map(\.toolbarIdentifier)
    }

    func toolbarSelectableItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] {
        SettingsPane.allCases.map(\.toolbarIdentifier)
    }

    func toolbar(
        _ toolbar: NSToolbar,
        itemForItemIdentifier itemIdentifier: NSToolbarItem.Identifier,
        willBeInsertedIntoToolbar flag: Bool
    ) -> NSToolbarItem? {
        guard let pane = SettingsPane(rawValue: itemIdentifier.rawValue) else { return nil }
        let item = NSToolbarItem(itemIdentifier: itemIdentifier)
        item.label = pane.title
        item.image = NSImage(systemSymbolName: pane.symbolName, accessibilityDescription: pane.title)
        item.target = self
        item.action = #selector(selectPane(_:))
        return item
    }
}

struct SettingsView: View {
    struct Actions {
        let clearHistory: () -> Void
        let requestPastePermission: () -> Void
    }

    @ObservedObject var selection: SettingsSelection
    @ObservedObject var preferences: Preferences
    let actions: Actions

    var body: some View {
        Group {
            switch selection.pane {
            case .general:
                GeneralSettings(preferences: preferences, actions: actions)
            case .privacy:
                PrivacySettings(preferences: preferences, actions: actions)
            }
        }
        .formStyle(.grouped)
        .scrollDisabled(true)
        .frame(width: 500)
        .fixedSize(horizontal: false, vertical: true)
    }
}

private struct GeneralSettings: View {
    @ObservedObject var preferences: Preferences
    let actions: SettingsView.Actions

    @State private var isTrusted = AXIsProcessTrusted()

    var body: some View {
        Form {
            Section {
                Toggle("Open Insert at login", isOn: $preferences.launchAtLogin)
                if let error = preferences.loginItemError {
                    SettingsWarning(text: error)
                }
                Toggle("Hide the Dock icon", isOn: $preferences.hideDockIcon)
            }

            Section {
                LabeledContent("Shortcut to open Insert") {
                    ShortcutRecorder(shortcut: $preferences.hotKeyShortcut)
                }
                if preferences.hotKeyIsUnavailable {
                    SettingsWarning(text: "Another app uses this shortcut. Record a different shortcut.")
                }
            }

            Section {
                Toggle(isOn: $preferences.pasteDirectly) {
                    Text("Paste directly into the active app")
                    Text("When this is off, Insert only copies the clip.")
                }
                if preferences.pasteDirectly && !isTrusted {
                    LabeledContent {
                        Button("Allow…", action: actions.requestPastePermission)
                    } label: {
                        SettingsWarning(text: "Insert needs the Accessibility permission to paste.")
                    }
                }
            }

            Section {
                Picker(selection: $preferences.historyLimit) {
                    ForEach(Preferences.historyLimitOptions, id: \.self) { limit in
                        Text("\(limit.formatted()) clips").tag(limit)
                    }
                } label: {
                    Text("Keep in history")
                    Text("Insert deletes the oldest clips first. Pinned clips do not count.")
                }
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)) { _ in
            isTrusted = AXIsProcessTrusted()
        }
    }
}

private struct PrivacySettings: View {
    @ObservedObject var preferences: Preferences
    let actions: SettingsView.Actions

    var body: some View {
        Form {
            Section {
                LabeledContent {
                    Button("Add App…", action: addIgnoredApp)
                } label: {
                    Text("Ignored apps")
                    Text("Insert does not save a copy that you make while one of these apps is in front.")
                }

                ForEach(preferences.ignoredApps, id: \.bundleID) { app in
                    LabeledContent {
                        Button {
                            preferences.ignoredApps.removeAll { $0.bundleID == app.bundleID }
                        } label: {
                            Image(systemName: "minus.circle.fill")
                                .foregroundStyle(.secondary)
                        }
                        .buttonStyle(.plain)
                        .help("Remove \(app.name)")
                    } label: {
                        HStack(spacing: 8) {
                            if let icon = AppIconStore.shared.entry(for: app).icon {
                                Image(nsImage: icon)
                                    .resizable()
                                    .frame(width: 20, height: 20)
                            }
                            Text(app.name)
                        }
                    }
                }
            } footer: {
                Text("Insert always skips items that an app marks as concealed or transient, such as passwords from a password manager.")
                    .frame(maxWidth: .infinity, alignment: .leading)
            }

            Section {
                Toggle(isOn: $preferences.capturePaused) {
                    Text("Pause capture")
                    Text("Insert saves no new clips while capture is paused.")
                }
            }

            Section {
                LabeledContent {
                    Button("Clear History…", action: actions.clearHistory)
                } label: {
                    Text("History")
                    Text("Delete all clips that are not pinned.")
                }
            }
        }
    }

    private func addIgnoredApp() {
        let panel = NSOpenPanel()
        panel.directoryURL = URL(fileURLWithPath: "/Applications")
        panel.allowedContentTypes = [.application]
        panel.allowsMultipleSelection = true
        panel.canChooseDirectories = false
        panel.prompt = "Ignore"
        guard panel.runModal() == .OK else { return }

        for url in panel.urls {
            guard let bundle = Bundle(url: url), let bundleID = bundle.bundleIdentifier else { continue }
            guard !preferences.ignoredApps.contains(where: { $0.bundleID == bundleID }) else { continue }
            let name = FileManager.default.displayName(atPath: url.path).replacingOccurrences(of: ".app", with: "")
            preferences.ignoredApps.append(SourceApp(bundleID: bundleID, name: name))
        }
    }
}

private struct SettingsWarning: View {
    let text: String

    var body: some View {
        Label(text, systemImage: "exclamationmark.triangle.fill")
            .font(.callout)
            .foregroundStyle(.orange)
    }
}

private struct ShortcutRecorder: View {
    @Binding var shortcut: GlobalShortcut

    @State private var monitor: Any?

    var body: some View {
        Button(monitor == nil ? shortcut.displayName : "Press a shortcut…") {
            monitor == nil ? startRecording() : stopRecording()
        }
        .help("Use at least one modifier key. Press Esc to cancel.")
        .onDisappear(perform: stopRecording)
    }

    private func startRecording() {
        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            if Int(event.keyCode) == kVK_Escape {
                stopRecording()
            } else if let recorded = GlobalShortcut(event: event) {
                shortcut = recorded
                stopRecording()
            } else {
                NSSound.beep()
            }
            return nil
        }
    }

    private func stopRecording() {
        if let monitor {
            NSEvent.removeMonitor(monitor)
        }
        monitor = nil
    }
}
