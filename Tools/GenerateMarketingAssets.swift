import AppKit
import Combine
import ScreenCaptureKit
import SwiftUI

// Records the real tray and the real Settings window with sample clips.
// The tray is in a stage window behind the other windows. ScreenCaptureKit captures only the windows of this tool.
// The tool does not read the general pasteboard and does not post keys. It needs the Screen Recording permission.

@main
@MainActor
enum MarketingAssets {
    static func main() {
        let output = URL(fileURLWithPath: CommandLine.arguments.dropFirst().first ?? "build/marketing", isDirectory: true)
        let app = NSApplication.shared
        app.setActivationPolicy(.accessory)

        let director = Director(output: output)
        Task { @MainActor in
            do {
                try await director.run()
                exit(0)
            } catch {
                FileHandle.standardError.write(Data("Could not make the marketing assets: \(error)\n".utf8))
                exit(1)
            }
        }
        app.run()
    }
}

@MainActor
private final class Stage: ObservableObject {
    @Published var keys: String?
    @Published var pastedText = ""

    private var keysToken = 0

    func show(keys: String) {
        keysToken += 1
        let token = keysToken
        self.keys = keys
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.9) { [weak self] in
            guard let self, self.keysToken == token else { return }
            self.keys = nil
        }
    }
}

@MainActor
private final class Director {
    private static let stageSize = NSSize(width: 1440, height: 810)
    private static let storageID = "com.local.Insert.marketing"

    private let output: URL
    private let stage = Stage()
    private let preferences = Preferences()
    private let library: ClipLibrary
    private let model: PanelModel
    private let window: NSWindow
    private let tray = TrayBackdropView()
    private var isTrayShown = false
    private var cancellables = Set<AnyCancellable>()

    init(output: URL) {
        self.output = output
        Self.removeStorage()
        library = ClipLibrary(storage: ClipStorage(bundleID: Self.storageID), historyLimit: 500)
        model = PanelModel(library: library)

        window = NSWindow(
            contentRect: NSRect(origin: .zero, size: Self.stageSize),
            styleMask: [.borderless],
            backing: .buffered,
            defer: false
        )
        window.appearance = NSAppearance(named: .darkAqua)
        window.ignoresMouseEvents = true
        window.collectionBehavior = [.canJoinAllSpaces, .stationary, .ignoresCycle]
        window.isReleasedWhenClosed = false

        let content = NSView(frame: NSRect(origin: .zero, size: Self.stageSize))
        let desktop = NSHostingView(rootView: DesktopView(stage: stage))
        desktop.frame = content.bounds
        content.addSubview(desktop)

        // The real tray is in a key panel. This environment value gives the same active colors in a window that is not key.
        let trayView = TrayView(model: model, library: library, preferences: preferences)
            .environment(\.controlActiveState, .key)
        let hostingView = NSHostingView(rootView: trayView)
        hostingView.sizingOptions = []
        hostingView.frame = tray.bounds
        hostingView.autoresizingMask = [.width, .height]
        tray.addSubview(hostingView)
        tray.isHidden = true
        content.addSubview(tray)
        window.contentView = content

        model.actions.activate = { [weak self] clip, _ in
            guard let self else { return }
            self.hideTray()
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) {
                self.stage.pastedText = self.library.payload(for: clip.id)?.plainText ?? ""
            }
        }

        model.$mode
            .removeDuplicates()
            .sink { [weak self] mode in
                self?.resizeTray(for: mode)
            }
            .store(in: &cancellables)
    }

    func run() async throws {
        defer { Self.removeStorage() }
        try FileManager.default.createDirectory(at: output, withIntermediateDirectories: true)
        let activity = ProcessInfo.processInfo.beginActivity(options: [.userInitiated, .latencyCritical], reason: "Record the demo")
        defer { ProcessInfo.processInfo.endActivity(activity) }

        try seedLibrary()

        // The stage is behind all other windows.
        center(window)
        window.order(.below, relativeTo: 0)
        await pause(0.5)

        let capture = try await WindowCapture(window: window, includesShadow: false)

        showTray()
        await pause(1.0)
        try await capture.writePNG(to: output.appendingPathComponent("insert-tray.png"))

        model.perform(.move(1))
        model.perform(.move(1))
        model.perform(.togglePreview)
        await pause(1.0)
        try await capture.writePNG(to: output.appendingPathComponent("insert-preview.png"))

        model.perform(.togglePreview)
        hideTray()
        await pause(0.6)

        try await capture.startRecording(to: output.appendingPathComponent("insert-demo-raw.mp4"))
        await playDemo()
        try await capture.stopRecording()

        try await captureSettings()
    }

    // MARK: Demo

    private func playDemo() async {
        await pause(0.8)
        stage.show(keys: "⇧ ⌘ V")
        showTray()
        await pause(1.4)

        await press("→", .move(1))
        await press("→", .move(1), hold: 0.7)
        await press("Space", .togglePreview, hold: 1.7)
        await press("→", .move(1), hold: 1.4)
        await press("Space", .togglePreview, hold: 0.8)

        await press("→", .move(1), hold: 0.6)
        await press("⌫", .deleteSelection, hold: 1.0)

        var typed = ""
        for character in "git" {
            typed.append(character)
            stage.show(keys: typed.map(String.init).joined(separator: " ").uppercased())
            model.query = typed
            await pause(0.22)
        }
        await pause(1.4)
        await press("Esc", .cancel, hold: 0.8)

        await press("Tab", .switchScope(1), hold: 1.1)
        await press("Tab", .switchScope(1), hold: 1.1)
        await press("Tab", .switchScope(1), hold: 0.8)

        stage.show(keys: "⌘")
        model.isCommandHeld = true
        await pause(1.1)
        stage.show(keys: "⌘ 1")
        model.perform(.activateIndex(0))
        await pause(2.4)
    }

    private func press(_ keys: String, _ command: PanelCommand, hold: Double = 0.45) async {
        stage.show(keys: keys)
        model.perform(command)
        await pause(hold)
    }

    private func pause(_ seconds: Double) async {
        try? await Task.sleep(nanoseconds: UInt64(seconds * 1_000_000_000))
    }

    // MARK: Tray

    // These three functions do the same animations as `PanelController`, with a view in place of the panel.

    private func trayFrame(for mode: PanelModel.Mode) -> NSRect {
        var height = TrayMetrics.baseHeight
        if mode == .previewing {
            height += min(Self.stageSize.height * 0.46, 480)
        }
        return NSRect(x: 0, y: 0, width: Self.stageSize.width, height: height)
    }

    private func showTray() {
        isTrayShown = true
        model.prepareToShow(needsPastePermission: false)

        let visibleFrame = trayFrame(for: model.mode)
        tray.frame = visibleFrame.offsetBy(dx: 0, dy: -visibleFrame.height)
        tray.alphaValue = 0
        tray.isHidden = false

        NSAnimationContext.runAnimationGroup { context in
            context.duration = 0.22
            context.timingFunction = CAMediaTimingFunction(name: .easeOut)
            tray.animator().frame = visibleFrame
            tray.animator().alphaValue = 1
        }
    }

    private func hideTray() {
        isTrayShown = false
        let hiddenFrame = tray.frame.offsetBy(dx: 0, dy: -tray.frame.height)
        NSAnimationContext.runAnimationGroup { context in
            context.duration = 0.16
            context.timingFunction = CAMediaTimingFunction(name: .easeIn)
            tray.animator().frame = hiddenFrame
            tray.animator().alphaValue = 0
        } completionHandler: { [weak self] in
            Task { @MainActor in
                guard let self, !self.isTrayShown else { return }
                self.tray.isHidden = true
                self.model.didHide()
            }
        }
    }

    private func resizeTray(for mode: PanelModel.Mode) {
        guard isTrayShown else { return }
        NSAnimationContext.runAnimationGroup { context in
            context.duration = 0.2
            context.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
            tray.animator().frame = trayFrame(for: mode)
        }
    }

    // MARK: Settings

    private func captureSettings() async throws {
        window.orderOut(nil)

        let known = Set(NSApp.windows.map(\.windowNumber))
        let controller = SettingsWindowController(
            preferences: preferences,
            actions: .init(clearHistory: {}, requestPastePermission: {})
        )
        controller.show()

        guard let settingsWindow = NSApp.windows.first(where: { $0.isVisible && !known.contains($0.windowNumber) }) else {
            throw AssetError.message("The Settings window did not open.")
        }
        center(settingsWindow)
        settingsWindow.makeFirstResponder(nil)
        await pause(1.0)

        let capture = try await WindowCapture(window: settingsWindow, includesShadow: true)
        try await capture.writePNG(to: output.appendingPathComponent("insert-settings.png"))
        settingsWindow.close()
    }

    /// The screen with the highest pixel density gives the sharpest capture.
    private func center(_ window: NSWindow) {
        let screen = NSScreen.screens.max { $0.backingScaleFactor < $1.backingScaleFactor } ?? NSScreen.main
        guard let frame = screen?.visibleFrame else { return }
        window.setFrameOrigin(NSPoint(x: frame.midX - window.frame.width / 2, y: frame.midY - window.frame.height / 2))
    }

    // MARK: Sample clips

    private func seedLibrary() throws {
        let files = FileManager.default.temporaryDirectory.appendingPathComponent("InsertMarketing", isDirectory: true)
        try FileManager.default.createDirectory(at: files, withIntermediateDirectories: true)
        let fileURLs = ["Launch plan.pdf", "Screenshots.zip"].map { files.appendingPathComponent($0) }
        for url in fileURLs {
            try Data().write(to: url)
        }

        let notes = SourceApp(bundleID: "com.apple.Notes", name: "Notes")
        let safari = SourceApp(bundleID: "com.apple.Safari", name: "Safari")
        let preview = SourceApp(bundleID: "com.apple.Preview", name: "Preview")
        let freeform = SourceApp(bundleID: "com.apple.freeform", name: "Freeform")
        let messages = SourceApp(bundleID: "com.apple.MobileSMS", name: "Messages")
        let finder = SourceApp(bundleID: "com.apple.finder", name: "Finder")
        let terminal = SourceApp(bundleID: "com.apple.Terminal", name: "Terminal")
        let mail = SourceApp(bundleID: "com.apple.mail", name: "Mail")

        let snippets = library.addPinboard(named: "Snippets")
        let design = library.addPinboard(named: "Design")
        library.setColor(.blue, forPinboard: snippets)
        library.setColor(.pink, forPinboard: design)

        // Oldest first. Each clip goes to the front of the history.
        let samples: [(minutesAgo: Double, source: SourceApp, pinboard: Pinboard.ID?, write: (NSPasteboard) -> Void)] = [
            (4_300, safari, design, { pasteboard in
                let item = NSPasteboardItem()
                item.setString("https://developer.apple.com/design/human-interface-guidelines", forType: .URL)
                item.setString("Human Interface Guidelines", forType: NSPasteboard.PasteboardType("public.url-name"))
                pasteboard.writeObjects([item])
            }),
            (1_500, mail, snippets, { pasteboard in
                let text = "Thank you for the report.\n\nThe correction is in the next build. Tell us if you see the problem again."
                let rich = NSAttributedString(string: text, attributes: [.font: NSFont.systemFont(ofSize: 13)])
                let item = NSPasteboardItem()
                item.setData(rich.rtf(from: NSRange(location: 0, length: rich.length)) ?? Data(), forType: .rtf)
                item.setString(text, forType: .string)
                pasteboard.writeObjects([item])
            }),
            (190, terminal, snippets, { $0.setString("git switch -c release/0.3\nmake dmg\ngit tag v0.3.0 && git push --tags", forType: .string) }),
            (125, finder, nil, { $0.writeObjects(fileURLs.map { $0 as NSURL }) }),
            (48, messages, nil, { $0.setString("Lunch at 12:30? The usual place.", forType: .string) }),
            (26, freeform, design, { $0.setString("#5B8DEF", forType: .string) }),
            (11, preview, design, { $0.setData(Self.sampleImage(), forType: .png) }),
            (4, safari, nil, { pasteboard in
                let item = NSPasteboardItem()
                item.setString("https://github.com/KenWuqianghao/Insert", forType: .URL)
                item.setString("Insert: a native clipboard tray for macOS", forType: NSPasteboard.PasteboardType("public.url-name"))
                pasteboard.writeObjects([item])
            }),
            (0.3, notes, nil, { $0.setString(
                "Insert 0.3 is ready.\n\n• A tray of cards at the bottom of the screen\n• Pinboards for the clips that you use frequently\n• A large preview when you press Space\n• Search, and a filter for each type of clip",
                forType: .string
            ) })
        ]

        // The reader gets a private pasteboard, so the general pasteboard does not change.
        let pasteboard = NSPasteboard.withUniqueName()
        defer { pasteboard.releaseGlobally() }

        for sample in samples {
            pasteboard.clearContents()
            sample.write(pasteboard)
            guard let capture = PasteboardReader.read(pasteboard, source: sample.source) else {
                throw AssetError.message("A sample clip from \(sample.source.name) was not readable.")
            }
            var clip = capture.clip
            clip.createdAt = Date(timeIntervalSinceNow: -sample.minutesAgo * 60)
            library.insert(CapturedClip(clip: clip, payload: capture.payload, thumbnail: capture.thumbnail))
            if let pinboard = sample.pinboard {
                library.setPinboard(pinboard, for: [clip.id])
            }
        }
    }

    private static func sampleImage() -> Data {
        let size = NSSize(width: 1600, height: 1000)
        let bitmap = NSBitmapImageRep(
            bitmapDataPlanes: nil,
            pixelsWide: Int(size.width),
            pixelsHigh: Int(size.height),
            bitsPerSample: 8,
            samplesPerPixel: 4,
            hasAlpha: true,
            isPlanar: false,
            colorSpaceName: .deviceRGB,
            bytesPerRow: 0,
            bitsPerPixel: 0
        )!

        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)

        NSGradient(colors: [
            NSColor(srgbRed: 0.99, green: 0.76, blue: 0.48, alpha: 1),
            NSColor(srgbRed: 0.96, green: 0.45, blue: 0.47, alpha: 1),
            NSColor(srgbRed: 0.36, green: 0.27, blue: 0.62, alpha: 1)
        ])?.draw(in: NSRect(origin: .zero, size: size), angle: -90)

        NSColor(srgbRed: 1, green: 0.93, blue: 0.75, alpha: 0.95).setFill()
        NSBezierPath(ovalIn: NSRect(x: 980, y: 520, width: 260, height: 260)).fill()

        let ridges: [(NSColor, [NSPoint])] = [
            (NSColor(srgbRed: 0.30, green: 0.22, blue: 0.50, alpha: 1), [
                NSPoint(x: 0, y: 330), NSPoint(x: 330, y: 560), NSPoint(x: 620, y: 380),
                NSPoint(x: 900, y: 610), NSPoint(x: 1250, y: 360), NSPoint(x: 1600, y: 520)
            ]),
            (NSColor(srgbRed: 0.18, green: 0.14, blue: 0.36, alpha: 1), [
                NSPoint(x: 0, y: 180), NSPoint(x: 260, y: 330), NSPoint(x: 560, y: 210),
                NSPoint(x: 880, y: 400), NSPoint(x: 1180, y: 230), NSPoint(x: 1600, y: 380)
            ])
        ]
        for (color, points) in ridges {
            let path = NSBezierPath()
            path.move(to: NSPoint(x: 0, y: 0))
            points.forEach(path.line(to:))
            path.line(to: NSPoint(x: size.width, y: 0))
            path.close()
            color.setFill()
            path.fill()
        }

        NSGraphicsContext.restoreGraphicsState()
        return bitmap.representation(using: .png, properties: [:]) ?? Data()
    }

    private static func removeStorage() {
        let applicationSupport = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
        if let folder = applicationSupport?.appendingPathComponent(storageID, isDirectory: true) {
            try? FileManager.default.removeItem(at: folder)
        }
    }
}

private enum AssetError: Error {
    case message(String)
}

// MARK: - Capture

@MainActor
private final class WindowCapture {
    private final class RecordingObserver: NSObject, SCRecordingOutputDelegate, @unchecked Sendable {
        let (finished, continuation) = AsyncStream<Void>.makeStream()

        func recordingOutputDidFinishRecording(_ recordingOutput: SCRecordingOutput) {
            continuation.yield()
        }

        func recordingOutput(_ recordingOutput: SCRecordingOutput, didFailWithError error: Error) {
            FileHandle.standardError.write(Data("The recording stopped: \(error)\n".utf8))
            continuation.yield()
        }
    }

    private let filter: SCContentFilter
    private let configuration = SCStreamConfiguration()
    private let observer = RecordingObserver()
    private var stream: SCStream?
    private var recordingOutput: SCRecordingOutput?

    init(window: NSWindow, includesShadow: Bool) async throws {
        let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: false)
        guard let target = content.windows.first(where: { $0.windowID == CGWindowID(window.windowNumber) }) else {
            throw AssetError.message("ScreenCaptureKit did not find the window.")
        }

        filter = SCContentFilter(desktopIndependentWindow: target)
        configuration.width = Int(filter.contentRect.width * CGFloat(filter.pointPixelScale))
        configuration.height = Int(filter.contentRect.height * CGFloat(filter.pointPixelScale))
        configuration.minimumFrameInterval = CMTime(value: 1, timescale: 60)
        configuration.showsCursor = false
        configuration.ignoreShadowsSingleWindow = !includesShadow
        configuration.capturesAudio = false
    }

    func writePNG(to url: URL) async throws {
        let image = try await SCScreenshotManager.captureImage(contentFilter: filter, configuration: configuration)
        guard let data = NSBitmapImageRep(cgImage: image).representation(using: .png, properties: [:]) else {
            throw AssetError.message("Could not encode \(url.lastPathComponent).")
        }
        try data.write(to: url)
    }

    func startRecording(to url: URL) async throws {
        try? FileManager.default.removeItem(at: url)

        let recording = SCRecordingOutputConfiguration()
        recording.outputURL = url
        recording.outputFileType = .mp4
        recording.videoCodecType = .h264

        let stream = SCStream(filter: filter, configuration: configuration, delegate: nil)
        let recordingOutput = SCRecordingOutput(configuration: recording, delegate: observer)
        try stream.addRecordingOutput(recordingOutput)
        try await stream.startCapture()

        self.stream = stream
        self.recordingOutput = recordingOutput
    }

    func stopRecording() async throws {
        guard let stream, let recordingOutput else { return }
        try stream.removeRecordingOutput(recordingOutput)

        for await _ in observer.finished { break }

        try await stream.stopCapture()
        self.stream = nil
        self.recordingOutput = nil
    }
}

// MARK: - Stage views

/// The same material and corners as the tray panel. The blend is in the window, because the desktop is a view of the stage.
private final class TrayBackdropView: NSVisualEffectView {
    init() {
        super.init(frame: .zero)
        material = .hudWindow
        blendingMode = .withinWindow
        state = .active
        wantsLayer = true
        layer?.cornerRadius = TrayMetrics.cornerRadius
        layer?.cornerCurve = .continuous
        layer?.maskedCorners = [.layerMinXMaxYCorner, .layerMaxXMaxYCorner]
        layer?.masksToBounds = true
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) is not used")
    }
}

private struct DesktopView: View {
    @ObservedObject var stage: Stage

    var body: some View {
        ZStack(alignment: .top) {
            wallpaper

            DocumentWindow(pastedText: stage.pastedText)
                .frame(width: 860, height: 540)
                .padding(.top, 62)

            if let keys = stage.keys {
                KeysPill(keys: keys)
                    .padding(.top, 14)
                    .transition(.opacity)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .animation(.easeOut(duration: 0.15), value: stage.keys)
    }

    private var wallpaper: some View {
        ZStack {
            LinearGradient(
                colors: [Color(red: 0.20, green: 0.16, blue: 0.44), Color(red: 0.07, green: 0.24, blue: 0.42)],
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            )
            Circle()
                .fill(Color(red: 0.98, green: 0.47, blue: 0.42))
                .frame(width: 620, height: 620)
                .blur(radius: 150)
                .offset(x: -520, y: 250)
            Circle()
                .fill(Color(red: 0.25, green: 0.78, blue: 0.80))
                .frame(width: 560, height: 560)
                .blur(radius: 150)
                .offset(x: 520, y: -160)
            Circle()
                .fill(Color(red: 0.62, green: 0.40, blue: 0.95))
                .frame(width: 460, height: 460)
                .blur(radius: 140)
                .offset(x: 360, y: 330)
        }
    }
}

/// A plain document window. It is the app that gets the paste in the demo.
private struct DocumentWindow: View {
    let pastedText: String

    var body: some View {
        VStack(spacing: 0) {
            ZStack {
                HStack(spacing: 8) {
                    ForEach([Color(red: 1, green: 0.37, blue: 0.34), Color(red: 1, green: 0.74, blue: 0.18), Color(red: 0.16, green: 0.79, blue: 0.25)], id: \.self) { color in
                        Circle()
                            .fill(color)
                            .frame(width: 12, height: 12)
                    }
                    Spacer()
                }
                Text("Team update")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(.secondary)
            }
            .padding(.horizontal, 14)
            .frame(height: 40)
            .background(Color(white: 0.19))

            VStack(alignment: .leading, spacing: 14) {
                Text("Team update")
                    .font(.system(size: 28, weight: .bold))
                Text("Hi all,")
                    .font(.system(size: 15))

                if !pastedText.isEmpty {
                    Text(pastedText)
                        .font(.system(size: 15))
                        .lineSpacing(5)
                        .transition(.opacity)
                }

                RoundedRectangle(cornerRadius: 1)
                    .fill(Color.accentColor)
                    .frame(width: 2, height: 20)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .padding(.horizontal, 44)
            .padding(.top, 30)
            .background(Color(white: 0.13))
        }
        .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(Color.white.opacity(0.14), lineWidth: 1))
        .shadow(color: .black.opacity(0.45), radius: 30, y: 16)
        .animation(.easeOut(duration: 0.18), value: pastedText)
    }
}

private struct KeysPill: View {
    let keys: String

    var body: some View {
        HStack(spacing: 6) {
            ForEach(Array(keys.split(separator: " ").enumerated()), id: \.offset) { _, key in
                Text(key)
                    .font(.system(size: 14, weight: .semibold, design: .rounded))
                    .padding(.horizontal, 8)
                    .frame(minWidth: 28, minHeight: 26)
                    .background(Color.white.opacity(0.16), in: RoundedRectangle(cornerRadius: 6, style: .continuous))
            }
        }
        .foregroundStyle(.white)
        .padding(5)
        .background(Color.black.opacity(0.55), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
    }
}
