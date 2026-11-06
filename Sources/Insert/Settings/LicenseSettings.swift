import AppKit
import SwiftUI

struct LicenseSettings: View {
    @ObservedObject var store: LicenseStore

    @State private var key = ""

    var body: some View {
        Form {
            Section {
                LabeledContent(store.state.summary) {
                    if store.license == nil {
                        Button("Buy Insert") {
                            NSWorkspace.shared.open(LicenseStore.buyURL)
                        }
                    } else {
                        Button("Deactivate", action: deactivate)
                    }
                }
            }

            if store.license == nil {
                Section {
                    HStack {
                        TextField("License key", text: $key, prompt: Text("Paste your license key"))
                            .labelsHidden()
                            .onSubmit(activate)
                        Button("Activate", action: activate)
                            .disabled(key.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    }
                    if let error = store.activationError {
                        SettingsWarning(text: error.message)
                    }
                }
            }
        }
    }

    private func activate() {
        if store.activate(key: key) {
            key = ""
        }
    }

    /// The field keeps the key, so the user can activate again without the email.
    private func deactivate() {
        key = store.license?.key ?? ""
        store.deactivate()
    }
}
