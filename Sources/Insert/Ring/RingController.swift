import AppKit
import Carbon
import SwiftUI

@MainActor
final class RingModel: ObservableObject {
    static let capacity = 8

    @Published private(set) var clips: [Clip] = []
    @Published private(set) var center = CGPoint.zero
    @Published private(set) var bounds = CGRect.zero
    @Published private(set) var selection: Int?
    @Published private(set) var showCount = 0

    func present(_ clips: [Clip], pointer: CGPoint, bounds: CGRect) -> CGPoint {
        let margin = RingGeometry.outerRadius + 8
        self.clips = clips
        self.bounds = bounds
        center = CGPoint(
            x: min(max(pointer.x, bounds.minX + margin), bounds.maxX - margin),
            y: min(max(pointer.y, bounds.minY + margin), bounds.maxY - margin)
        )
        selection = nil
        showCount += 1
        return center
    }

    func dismiss() {
        selection = nil
    }

    func track(_ point: CGPoint) {
        if isOnPreviewCard(point) {
            return
        }
        let index = RingGeometry.segment(at: point, center: center, count: clips.count)
        if index != selection {
            selection = index
        }
    }

    func acceptsClick(at point: CGPoint) -> Bool {
        guard let selection else { return false }
        return segmentArea(contains: point) || previewFrame(for: selection).contains(point)
    }

    private func isOnPreviewCard(_ point: CGPoint) -> Bool {
        guard let selection else { return false }
        return previewFrame(for: selection).contains(point) && !segmentArea(contains: point)
    }

    private func segmentArea(contains point: CGPoint) -> Bool {
        let distance = hypot(point.x - center.x, point.y - center.y)
        return (RingGeometry.innerRadius...RingGeometry.outerRadius).contains(distance)
    }

    private func previewFrame(for index: Int) -> CGRect {
        let side = TrayMetrics.cardSize
        let center = previewCenter(for: index)
        return CGRect(x: center.x - side / 2, y: center.y - side / 2, width: side, height: side)
    }

    func previewCenter(for index: Int) -> CGPoint {
        let angle = RingGeometry.direction(of: index, count: clips.count)
        let toward = previewPlacement(angle: angle)
        let away = previewPlacement(angle: angle + .pi)
        return toward.fits || !away.fits ? toward.center : away.center
    }

    private func previewPlacement(angle: Double) -> (center: CGPoint, fits: Bool) {
        let half = TrayMetrics.cardSize / 2
        let cardExtent = half * (abs(cos(angle)) + abs(sin(angle)))
        let reach = RingGeometry.outerRadius + RingGeometry.previewGap + cardExtent
        let wanted = CGPoint(x: center.x + cos(angle) * reach, y: center.y + sin(angle) * reach)
        let inset = half + 12
        let placed = CGPoint(
            x: min(max(wanted.x, bounds.minX + inset), bounds.maxX - inset),
            y: min(max(wanted.y, bounds.minY + inset), bounds.maxY - inset)
        )
        return (placed, placed == wanted)
    }
}

@MainActor
final class RingController: NSObject, NSWindowDelegate {
    private enum Presentation {
        case hidden
        case shown
        case hiding
    }

    private static let digitKeys = [
        [kVK_ANSI_1, kVK_ANSI_2, kVK_ANSI_3, kVK_ANSI_4, kVK_ANSI_5, kVK_ANSI_6, kVK_ANSI_7, kVK_ANSI_8],
        [kVK_ANSI_Keypad1, kVK_ANSI_Keypad2, kVK_ANSI_Keypad3, kVK_ANSI_Keypad4,
         kVK_ANSI_Keypad5, kVK_ANSI_Keypad6, kVK_ANSI_Keypad7, kVK_ANSI_Keypad8]
    ]

    private let model = RingModel()
    private let library: ClipLibrary
    private let pasteService: PasteService
    private let panel = RingPanel()
    private var presentation = Presentation.hidden

    init(library: ClipLibrary, pasteService: PasteService) {
        self.library = library
        self.pasteService = pasteService
        super.init()

        let hostingView = RingHostingView(rootView: RingView(model: model, library: library))
        hostingView.sizingOptions = []
        hostingView.onMove = { [weak self] point in
            self?.pointerMoved(to: point)
        }
        hostingView.onClick = { [weak self] point in
            self?.clicked(at: point)
        }
        hostingView.onRightClick = { [weak self] in
            self?.hide()
        }
        panel.contentView = hostingView
        panel.delegate = self

        NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            guard let self, self.panel.isKeyWindow else { return event }
            self.keyDown(event)
            return nil
        }
    }

    private var isShown: Bool {
        presentation == .shown
    }

    func endHold() {
        guard isShown else { return }
        model.track(Self.ringPoint(of: NSEvent.mouseLocation, on: panel.frame))
        if let index = model.selection {
            paste(index)
        } else {
            hide()
        }
    }

    func show() -> Bool {
        guard presentation == .hidden else { return false }
        let clips = Array(library.clips.prefix(RingModel.capacity))
        guard !clips.isEmpty else { return false }

        let mouse = NSEvent.mouseLocation
        guard let screen = NSScreen.screens.first(where: { NSMouseInRect(mouse, $0.frame, false) }) ?? NSScreen.main else {
            return false
        }

        let frame = screen.frame
        let visible = screen.visibleFrame
        let pointer = Self.ringPoint(of: mouse, on: frame)
        let center = model.present(
            clips,
            pointer: pointer,
            bounds: CGRect(x: visible.minX - frame.minX, y: frame.maxY - visible.maxY, width: visible.width, height: visible.height)
        )
        if center != pointer {
            movePointer(to: center, on: screen)
        }

        panel.setFrame(frame, display: false)
        panel.alphaValue = 1
        panel.makeKeyAndOrderFront(nil)
        presentation = .shown
        return true
    }

    func hide(then completion: (() -> Void)? = nil) {
        guard isShown else { return }
        presentation = .hiding

        NSAnimationContext.runAnimationGroup { context in
            context.duration = 0.12
            panel.animator().alphaValue = 0
        } completionHandler: { [weak self] in
            Task { @MainActor in
                guard let self else { return }
                self.panel.orderOut(nil)
                self.model.dismiss()
                self.presentation = .hidden
                completion?()
            }
        }
    }

    func windowDidResignKey(_ notification: Notification) {
        hide()
    }

    private func pointerMoved(to point: CGPoint) {
        if isShown {
            model.track(point)
        }
    }

    /// Converts a point in AppKit screen coordinates to the coordinates of the ring view on the screen with `frame`.
    private static func ringPoint(of point: CGPoint, on frame: CGRect) -> CGPoint {
        CGPoint(x: point.x - frame.minX, y: frame.maxY - point.y)
    }

    /// `point` uses the coordinates of the ring view on `screen`.
    private func movePointer(to point: CGPoint, on screen: NSScreen) {
        guard let primary = NSScreen.screens.first else { return }
        // Quartz puts the origin at the top left of the primary screen. AppKit puts it at the bottom left.
        let frame = screen.frame
        CGWarpMouseCursorPosition(CGPoint(x: frame.minX + point.x, y: primary.frame.maxY - frame.maxY + point.y))
        CGAssociateMouseAndMouseCursorPosition(1)
    }

    private func clicked(at point: CGPoint) {
        guard isShown else { return }
        if let index = model.selection, model.acceptsClick(at: point) {
            paste(index)
        } else {
            hide()
        }
    }

    private func keyDown(_ event: NSEvent) {
        let keyCode = Int(event.keyCode)
        switch keyCode {
        case kVK_Escape:
            hide()
        case kVK_Return, kVK_ANSI_KeypadEnter:
            if let index = model.selection {
                paste(index)
            }
        default:
            if let index = Self.digitKeys.lazy.compactMap({ $0.firstIndex(of: keyCode) }).first {
                paste(index)
            } else {
                hide()
            }
        }
    }

    private func paste(_ index: Int) {
        guard model.clips.indices.contains(index) else { return }
        let clip = model.clips[index]
        hide { [weak self] in
            if self?.pasteService.perform(.paste, with: clip) != true {
                NSSound.beep()
            }
        }
    }
}

private final class RingPanel: NSPanel {
    init() {
        super.init(
            contentRect: .zero,
            styleMask: [.borderless, .nonactivatingPanel, .fullSizeContentView],
            backing: .buffered,
            defer: true
        )

        isOpaque = false
        // The window server sends a click on a fully clear pixel to the window below.
        // An almost clear background keeps every click and pointer move in the ring panel.
        backgroundColor = NSColor(white: 0, alpha: 0.01)
        hasShadow = false
        level = .popUpMenu
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .transient, .ignoresCycle]
        isReleasedWhenClosed = false
        hidesOnDeactivate = false
        isMovable = false
        animationBehavior = .none
        acceptsMouseMovedEvents = true
    }

    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }
}

private final class RingHostingView: NSHostingView<RingView> {
    var onMove: (CGPoint) -> Void = { _ in }
    var onClick: (CGPoint) -> Void = { _ in }
    var onRightClick: () -> Void = {}

    private var pointerArea: NSTrackingArea?

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        guard pointerArea == nil else { return }
        let area = NSTrackingArea(rect: .zero, options: [.mouseMoved, .activeAlways, .inVisibleRect], owner: self)
        addTrackingArea(area)
        pointerArea = area
    }

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    override func mouseMoved(with event: NSEvent) {
        onMove(location(of: event))
    }

    override func mouseDragged(with event: NSEvent) {
        onMove(location(of: event))
    }

    override func mouseDown(with event: NSEvent) {
        onClick(location(of: event))
    }

    override func rightMouseDown(with event: NSEvent) {
        onRightClick()
    }

    private func location(of event: NSEvent) -> CGPoint {
        let point = convert(event.locationInWindow, from: nil)
        return isFlipped ? point : CGPoint(x: point.x, y: bounds.height - point.y)
    }
}
