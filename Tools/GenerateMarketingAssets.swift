import AppKit
import AVFoundation
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

private struct Caption: Equatable {
    let keys: [String]
    let label: String
}

@MainActor
private final class Stage: ObservableObject {
    @Published var caption: Caption?
    @Published var pastedText = ""

    private var captionToken = 0

    func show(_ keys: [String], _ label: String, for seconds: Double = 1.3) {
        captionToken += 1
        let token = captionToken
        caption = Caption(keys: keys, label: label)
        DispatchQueue.main.asyncAfter(deadline: .now() + seconds) { [weak self] in
            guard let self, self.captionToken == token else { return }
            self.caption = nil
        }
    }
}

@MainActor
private final class Director {
    /// The size of a 14-inch MacBook Pro screen at the default scale, so the video fits a laptop frame.
    static let stageSize = NSSize(width: 1512, height: 945)
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
        NSApp.appearance = NSAppearance(named: .aqua)
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

        // macOS slows the drawing of a window that other windows cover, and the recording then loses frames.
        // So the stage is in front of the other windows while the tool runs. It ignores the mouse.
        center(window)
        window.level = .floating
        window.orderFrontRegardless()
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

        try await recordDemo(with: capture)

        try await captureSettings()
    }

    // MARK: Demo

    /// One key press in the demo. A step with a chapter name starts a part of the demo that the landing page can seek to.
    private struct Step {
        let seconds: Double
        let animates: Bool
        let chapter: String?
    }

    private struct Chapter: Encodable {
        let name: String
        let at: Double
    }

    private var steps: [Step] = []
    private var recordingStart = Date()

    /// ScreenCaptureKit drops frames when the Mac is busy, and the video then skips animations.
    /// Each take is checked. If frames are missing, the tool records the demo again.
    private func recordDemo(with capture: WindowCapture) async throws {
        let url = output.appendingPathComponent("insert-demo-raw.mp4")
        let takes = 3
        for take in 1...takes {
            stage.pastedText = ""
            steps = []
            await pause(0.6)

            try await capture.startRecording(to: url)
            recordingStart = Date()
            await playDemo()
            try await capture.stopRecording()

            let frames = try await frameTimes(in: url)
            let incomplete = steps.filter { step in
                let soon = frames.filter { $0 >= step.seconds && $0 < step.seconds + 0.4 }.count
                // A tray animation is about 0.25 s, so a complete one gives at least 10 frames at 60 fps.
                return soon < (step.animates ? 10 : 1)
            }
            if incomplete.isEmpty {
                try writeChapters(frames: frames)
                return
            }
            let times = incomplete.map { String(format: "%.1f s", $0.seconds) }.joined(separator: ", ")
            FileHandle.standardError.write(Data("Take \(take) of \(takes) lost frames after the steps at \(times).\n".utf8))
        }
        throw AssetError.message("Each take of the demo lost frames. Unlock the screen and keep the display on, close busy apps, and try again.")
    }

    private func frameTimes(in url: URL) async throws -> [Double] {
        let asset = AVURLAsset(url: url)
        guard let track = try await asset.loadTracks(withMediaType: .video).first else {
            throw AssetError.message("The demo recording has no video track.")
        }
        let reader = try AVAssetReader(asset: asset)
        let trackOutput = AVAssetReaderTrackOutput(track: track, outputSettings: nil)
        reader.add(trackOutput)
        reader.startReading()
        var sampleTimes: [Double] = []
        while let sample = trackOutput.copyNextSampleBuffer() {
            if CMSampleBufferGetNumSamples(sample) > 0 {
                sampleTimes.append(CMSampleBufferGetPresentationTimeStamp(sample).seconds)
            }
        }
        guard reader.status == .completed else {
            throw AssetError.message("Could not read the demo recording: \(String(describing: reader.error))")
        }
        // The sample times do not start at zero. The first frame of the video is the first frame of the recording.
        let start = sampleTimes.min() ?? 0
        return sampleTimes.map { $0 - start }.sorted()
    }

    /// Each chapter starts at the first frame of its step, so a seek shows the new caption and not the previous one.
    private func writeChapters(frames: [Double]) throws {
        let chapters = steps.compactMap { step -> Chapter? in
            guard let name = step.chapter, let frame = frames.first(where: { $0 >= step.seconds }) else { return nil }
            return Chapter(name: name, at: (frame * 100).rounded(.up) / 100)
        }
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted]
        try encoder.encode(chapters).write(to: output.appendingPathComponent("insert-demo-chapters.json"))
    }

    private func playDemo() async {
        await pause(0.9)

        cue(["⇧", "⌘", "V"], "Open Insert", for: 1.6, animates: true, chapter: "Open")
        showTray()
        await pause(1.6)

        await press(["→"], "Move", .move(1), hold: 0.5)
        await press(["→"], "Move", .move(1), hold: 0.6)

        await press(["Space"], "Preview", .togglePreview, hold: 1.8, animates: true, chapter: "Preview")
        await press(["→"], "Move", .move(1), hold: 1.3)
        await press(["Space"], "Close the preview", .togglePreview, hold: 0.9, animates: true)

        var typed = ""
        for character in "git" {
            typed.append(character)
            cue(typed.uppercased().map(String.init), "Search", for: 1.6, chapter: typed.count == 1 ? "Search" : nil)
            model.query = typed
            await pause(0.24)
        }
        await pause(1.5)
        await press(["Esc"], "Clear the search", .cancel, hold: 0.8)

        await press(["Tab"], "Next pinboard", .switchScope(1), hold: 1.2, animates: true, chapter: "Pinboards")
        await press(["Tab"], "Next pinboard", .switchScope(1), hold: 1.2, animates: true)
        await press(["Tab"], "Next pinboard", .switchScope(1), hold: 0.8, animates: true)

        cue(["⌘"], "Quick paste", for: 1.2, chapter: "Paste")
        model.isCommandHeld = true
        await pause(1.1)
        cue(["⌘", "1"], "Paste the first card", for: 1.4, animates: true)
        model.perform(.activateIndex(0))
        await pause(2.8)
    }

    private func press(_ keys: [String], _ label: String, _ command: PanelCommand, hold: Double, animates: Bool = false, chapter: String? = nil) async {
        cue(keys, label, animates: animates, chapter: chapter)
        model.perform(command)
        await pause(hold)
    }

    private func cue(_ keys: [String], _ label: String, for seconds: Double = 1.3, animates: Bool = false, chapter: String? = nil) {
        steps.append(Step(seconds: Date().timeIntervalSince(recordingStart), animates: animates, chapter: chapter))
        stage.show(keys, label, for: seconds)
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
            (190, terminal, snippets, { $0.setString("git switch -c release\nmake dmg\ngh release create --generate-notes", forType: .string) }),
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
                "What is new in Insert:\n\n• A tray of cards at the bottom of the screen\n• Pinboards for the clips that you use frequently\n• A large preview when you press Space\n• Search, and a filter for each type of clip",
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
            Image(nsImage: Wallpaper.image)
                .resizable()

            ComposeWindow(pastedText: stage.pastedText)
                .frame(width: 820, height: 470)
                .padding(.top, 104)

            MenuBar()

            if let caption = stage.caption {
                CaptionView(caption: caption)
                    .padding(.top, 40)
                    .transition(.opacity.combined(with: .scale(scale: 0.96)))
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        // Only the show and the hide animate. A new caption replaces the old one at once, so the two labels do not overlap.
        .animation(.easeOut(duration: 0.16), value: stage.caption == nil)
    }
}

/// The stage shows it as a still image. With live blur filters, the tray backdrop drew too slowly and the recording lost frames.
private struct Wallpaper: View {
    @MainActor static let image: NSImage = {
        let renderer = ImageRenderer(content: Wallpaper().frame(width: Director.stageSize.width, height: Director.stageSize.height))
        renderer.scale = 2
        return renderer.nsImage ?? NSImage()
    }()

    var body: some View {
        GeometryReader { proxy in
            let size = proxy.size
            ZStack {
                LinearGradient(
                    colors: [Color(red: 1.0, green: 0.72, blue: 0.30), Color(red: 0.99, green: 0.44, blue: 0.10), Color(red: 0.86, green: 0.20, blue: 0.08)],
                    startPoint: .topTrailing,
                    endPoint: .bottomLeading
                )
                Fold(phase: 0.0, depth: 0.32)
                    .fill(LinearGradient(colors: [Color(red: 1.0, green: 0.88, blue: 0.60).opacity(0.85), .clear], startPoint: .top, endPoint: .bottom))
                    .blur(radius: 18)
                Fold(phase: 1.3, depth: 0.55)
                    .fill(LinearGradient(colors: [Color(red: 1.0, green: 0.55, blue: 0.20).opacity(0.9), Color(red: 0.86, green: 0.30, blue: 0.10).opacity(0.0)], startPoint: .top, endPoint: .bottom))
                    .blur(radius: 10)
                Fold(phase: 2.4, depth: 0.74)
                    .fill(LinearGradient(colors: [Color(red: 0.95, green: 0.38, blue: 0.12).opacity(0.95), Color(red: 0.62, green: 0.14, blue: 0.08)], startPoint: .top, endPoint: .bottom))
                    .blur(radius: 6)
                Circle()
                    .fill(Color(red: 1.0, green: 0.93, blue: 0.70))
                    .frame(width: size.width * 0.5)
                    .blur(radius: 160)
                    .offset(x: size.width * 0.32, y: -size.height * 0.42)
                    .opacity(0.8)
            }
        }
    }
}

private struct Fold: Shape {
    let phase: Double
    let depth: Double

    func path(in rect: CGRect) -> Path {
        var path = Path()
        let top = rect.height * depth
        path.move(to: CGPoint(x: 0, y: rect.height))
        for step in 0...64 {
            let x = rect.width * Double(step) / 64
            let angle = Double(step) / 64 * .pi * 1.6 + phase
            let y = top + sin(angle) * rect.height * 0.09 + cos(angle * 0.5) * rect.height * 0.05
            path.addLine(to: CGPoint(x: x, y: y))
        }
        path.addLine(to: CGPoint(x: rect.width, y: rect.height))
        path.closeSubpath()
        return path
    }
}

private struct MenuBar: View {
    var body: some View {
        HStack(spacing: 20) {
            Text("Mail").fontWeight(.bold)
            ForEach(["File", "Edit", "View", "Mailbox", "Message", "Format", "Window", "Help"], id: \.self) { Text($0) }
            Spacer()
            Image(systemName: "doc.on.clipboard")
            Image(systemName: "wifi")
            Image(systemName: "battery.75percent")
            Image(systemName: "magnifyingglass")
            Text("Thu Nov 6  9:41")
        }
        .font(.system(size: 13, weight: .medium))
        .foregroundStyle(Color.black.opacity(0.85))
        .padding(.horizontal, 22)
        .frame(height: 30)
        .background(Color.white.opacity(0.32))
    }
}

/// The app that gets the paste in the demo.
private struct ComposeWindow: View {
    let pastedText: String

    private let ink = Color(red: 0.11, green: 0.11, blue: 0.12)
    private let secondary = Color(red: 0.11, green: 0.11, blue: 0.12).opacity(0.5)
    private let rule = Color.black.opacity(0.08)

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 8) {
                ForEach([Color(red: 1, green: 0.37, blue: 0.34), Color(red: 1, green: 0.74, blue: 0.18), Color(red: 0.16, green: 0.79, blue: 0.25)], id: \.self) { color in
                    Circle()
                        .fill(color)
                        .frame(width: 12, height: 12)
                }
                Spacer()
                Image(systemName: "paperplane.fill")
                    .font(.system(size: 15))
                    .foregroundStyle(secondary)
            }
            .padding(.horizontal, 16)
            .frame(height: 46)

            field("To:", "Design team")
            field("Subject:", "Release notes")

            VStack(alignment: .leading, spacing: 12) {
                Text("Hi all,")
                Text("Here are the notes for the release:")

                if !pastedText.isEmpty {
                    Text(pastedText)
                        .lineSpacing(5)
                        .transition(.opacity.combined(with: .move(edge: .top)))
                }

                RoundedRectangle(cornerRadius: 1)
                    .fill(Color.accentColor)
                    .frame(width: 2, height: 19)
            }
            .font(.system(size: 15))
            .foregroundStyle(ink)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .padding(.horizontal, 24)
            .padding(.top, 20)
        }
        .background(Color.white)
        .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(Color.black.opacity(0.10), lineWidth: 0.5))
        .shadow(color: Color(red: 0.45, green: 0.12, blue: 0.0).opacity(0.35), radius: 40, y: 22)
        .animation(.easeOut(duration: 0.22), value: pastedText)
    }

    private func field(_ name: String, _ value: String) -> some View {
        VStack(spacing: 0) {
            HStack(spacing: 6) {
                Text(name).foregroundStyle(secondary)
                Text(value).foregroundStyle(ink)
                Spacer()
            }
            .font(.system(size: 14))
            .padding(.horizontal, 24)
            .frame(height: 38)
            rule.frame(height: 1).padding(.leading, 24)
        }
    }
}

private struct CaptionView: View {
    let caption: Caption

    var body: some View {
        HStack(spacing: 14) {
            HStack(spacing: 6) {
                ForEach(Array(caption.keys.enumerated()), id: \.offset) { _, key in
                    Text(key)
                        .font(.system(size: 20, weight: .semibold, design: .rounded))
                        .foregroundStyle(Color(white: 0.12))
                        .padding(.horizontal, 10)
                        .frame(minWidth: 38, minHeight: 38)
                        .background(
                            RoundedRectangle(cornerRadius: 8, style: .continuous)
                                .fill(Color.white)
                                .shadow(color: .black.opacity(0.35), radius: 0, y: 2)
                        )
                }
            }
            Text(caption.label)
                .font(.system(size: 19, weight: .semibold))
                .foregroundStyle(.white)
                .padding(.trailing, 6)
        }
        .padding(.vertical, 9)
        .padding(.horizontal, 10)
        .background(Color.black.opacity(0.72), in: Capsule())
        .shadow(color: .black.opacity(0.25), radius: 18, y: 8)
    }
}
