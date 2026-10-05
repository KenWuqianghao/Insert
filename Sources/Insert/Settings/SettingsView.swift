import AppKit
import ApplicationServices
import Carbon
import SwiftUI
import UniformTypeIdentifiers

@MainActor
final class SettingsWindowController {
    private let preferences: Preferences
    private let actions: SettingsView.Actions
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
        let window = NSWindow(contentViewController: NSHostingController(
            rootView: SettingsView(preferences: preferences, actions: actions)
        ))
        window.title = "Insert Settings"
        window.styleMask = [.titled, .closable]
        window.isReleasedWhenClosed = false
        window.center()
        return window
    }
}

struct SettingsView: View {
    struct Actions {
        let clearHistory: () -> Void
        let requestPastePermission: () -> Void
    }

    @ObservedObject var preferences: Preferences
    let actions: Actions

    var body: some View {
        TabView {
            GeneralSettings(preferences: preferences, actions: actions)
                .tabItem { Label("General", systemImage: "gearshape") }

            PrivacySettings(preferences: preferences, actions: actions)
                .tabItem { Label("Privacy", systemImage: "hand.raised") }
        }
        .frame(width: 480, height: 400)
        .padding(.top, 8)
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
                    Text(error)
                        .font(.caption)
                        .foregroundStyle(.red)
                }
                Toggle("Hide the Dock icon", isOn: $preferences.hideDockIcon)
            }

            Section {
                LabeledContent("Shortcut to open Insert") {
                    ShortcutRecorder(shortcut: $preferences.hotKeyShortcut)
                }
                if preferences.hotKeyIsUnavailable {
                    Label("Another app uses this shortcut. Record a different shortcut.", systemImage: "exclamationmark.triangle.fill")
                        .font(.caption)
                        .foregroundStyle(.orange)
                }
            }

            Section {
                Toggle("Paste directly into the active app", isOn: $preferences.pasteDirectly)
                if preferences.pasteDirectly && !isTrusted {
                    HStack {
                        Label("Insert needs the Accessibility permission to paste. Without it, Insert only copies.", systemImage: "exclamationmark.triangle.fill")
                            .font(.caption)
                            .foregroundStyle(.orange)
                        Spacer()
                        Button("Allow…", action: actions.requestPastePermission)
                    }
                }
            }

            Section {
                Picker("Keep in history", selection: $preferences.historyLimit) {
                    ForEach(Preferences.historyLimitOptions, id: \.self) { limit in
                        Text("\(limit.formatted()) clips").tag(limit)
                    }
                }
            } footer: {
                Text("Pinned clips do not count. Insert deletes the oldest clips when the history is full.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .formStyle(.grouped)
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
                if preferences.ignoredApps.isEmpty {
                    Text("No ignored apps")
                        .foregroundStyle(.secondary)
                }

                ForEach(preferences.ignoredApps, id: \.bundleID) { app in
                    HStack(spacing: 8) {
                        if let icon = AppIconStore.shared.entry(for: app).icon {
                            Image(nsImage: icon)
                                .resizable()
                                .frame(width: 20, height: 20)
                        }
                        Text(app.name)
                        Spacer()
                        Button {
                            preferences.ignoredApps.removeAll { $0.bundleID == app.bundleID }
                        } label: {
                            Image(systemName: "minus.circle.fill")
                        }
                        .buttonStyle(.plain)
                        .foregroundStyle(.secondary)
                        .help("Remove \(app.name)")
                    }
                }

                Button("Add App…", action: addIgnoredApp)
            } header: {
                Text("Ignored Apps")
            } footer: {
                Text("Insert does not save a copy that you make while one of these apps is in front. Insert also skips items that an app marks as concealed or transient, such as passwords from a password manager.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }

            Section {
                Toggle("Pause capture", isOn: $preferences.capturePaused)
                LabeledContent("Delete all clips that are not pinned") {
                    Button("Clear History…", action: actions.clearHistory)
                }
            }
        }
        .formStyle(.grouped)
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
