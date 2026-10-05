import AppKit
import Combine
import SwiftUI

@MainActor
final class PanelController: NSObject, NSWindowDelegate {
    private enum Presentation {
        case hidden
        case shown
        case hiding
    }

    private let model: PanelModel
    private let pasteService: PasteService
    private let panel = TrayPanel()
    private var presentation = Presentation.hidden
    private var screenFrame = NSRect.zero
    private var eventMonitors: [Any] = []
    private var cancellables = Set<AnyCancellable>()

    init(model: PanelModel, preferences: Preferences, pasteService: PasteService) {
        self.model = model
        self.pasteService = pasteService
        super.init()

        let background = TrayBackgroundView()
        let hostingView = NSHostingView(rootView: TrayView(model: model, library: model.library, preferences: preferences))
        hostingView.sizingOptions = []
        hostingView.frame = background.bounds
        hostingView.autoresizingMask = [.width, .height]
        background.addSubview(hostingView)
        panel.contentView = background
        panel.delegate = self

        model.actions.hide = { [weak self] in
            self?.hide()
        }
        model.actions.activate = { [weak self] clip, action in
            self?.hide {
                if self?.pasteService.perform(action, with: clip) != true {
                    NSSound.beep()
                }
            }
        }
        model.actions.requestPastePermission = { [weak self] in
            self?.hide()
            self?.pasteService.requestPermission()
        }

        model.$mode
            .removeDuplicates()
            .sink { [weak self] mode in
                self?.resize(for: mode)
            }
            .store(in: &cancellables)

        installEventMonitors()
    }

    func toggle() {
        presentation == .shown ? hide() : show()
    }

    func show() {
        guard presentation != .shown else { return }
        presentation = .shown

        let mouse = NSEvent.mouseLocation
        let screen = NSScreen.screens.first { NSMouseInRect(mouse, $0.frame, false) } ?? NSScreen.main
        screenFrame = screen?.frame ?? NSRect(x: 0, y: 0, width: 1440, height: 900)

        model.prepareToShow(needsPastePermission: pasteService.needsPermission)

        let visibleFrame = frame(for: model.mode)
        panel.setFrame(visibleFrame.offsetBy(dx: 0, dy: -visibleFrame.height), display: false)
        panel.alphaValue = 0
        panel.makeKeyAndOrderFront(nil)

        NSAnimationContext.runAnimationGroup { context in
            context.duration = 0.22
            context.timingFunction = CAMediaTimingFunction(name: .easeOut)
            panel.animator().setFrame(visibleFrame, display: true)
            panel.animator().alphaValue = 1
        }
    }

    func hide(then completion: (() -> Void)? = nil) {
        guard presentation == .shown else { return }
        presentation = .hiding

        let hiddenFrame = panel.frame.offsetBy(dx: 0, dy: -panel.frame.height)
        NSAnimationContext.runAnimationGroup { context in
            context.duration = 0.16
            context.timingFunction = CAMediaTimingFunction(name: .easeIn)
            panel.animator().setFrame(hiddenFrame, display: true)
            panel.animator().alphaValue = 0
        } completionHandler: { [weak self] in
            Task { @MainActor in
                guard let self, self.presentation == .hiding else { return }
                self.panel.orderOut(nil)
                self.presentation = .hidden
                self.model.didHide()
                completion?()
            }
        }
    }

    func windowDidResignKey(_ notification: Notification) {
        hide()
    }

    private func frame(for mode: PanelModel.Mode) -> NSRect {
        var height = TrayMetrics.baseHeight
        if mode == .previewing {
            height += min(screenFrame.height * 0.46, 480)
        }
        return NSRect(x: screenFrame.minX, y: screenFrame.minY, width: screenFrame.width, height: height)
    }

    private func resize(for mode: PanelModel.Mode) {
        guard presentation == .shown else { return }
        NSAnimationContext.runAnimationGroup { context in
            context.duration = 0.2
            context.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
            panel.animator().setFrame(frame(for: mode), display: true)
        }
    }

    private func installEventMonitors() {
        let keyMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            guard let self, self.panel.isKeyWindow else { return event }

            // An input method uses Return, Space, the arrows and Escape while it composes text.
            if let editor = self.panel.firstResponder as? NSTextView, editor.hasMarkedText() {
                return event
            }

            let context = PanelCommand.Context(mode: self.model.mode, queryIsEmpty: self.model.query.isEmpty)
            guard let command = PanelCommand(event: event, context: context) else { return event }
            self.model.perform(command)
            return nil
        }

        let flagsMonitor = NSEvent.addLocalMonitorForEvents(matching: .flagsChanged) { [weak self] event in
            guard let self else { return event }
            let isHeld = self.panel.isKeyWindow && event.modifierFlags.contains(.command)
            if self.model.isCommandHeld != isHeld {
                self.model.isCommandHeld = isHeld
            }
            return event
        }

        let outsideClickMonitor = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) { [weak self] _ in
            Task { @MainActor in
                self?.hide()
            }
        }

        eventMonitors = [keyMonitor, flagsMonitor, outsideClickMonitor].compactMap { $0 }
    }
}

/// A non-activating panel takes the keyboard but leaves the other app active, so a paste goes to that app.
private final class TrayPanel: NSPanel {
    init() {
        super.init(
            contentRect: .zero,
            styleMask: [.borderless, .nonactivatingPanel, .fullSizeContentView],
            backing: .buffered,
            defer: true
        )

        isOpaque = false
        backgroundColor = .clear
        hasShadow = true
        level = .mainMenu
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .transient, .ignoresCycle]
        isReleasedWhenClosed = false
        hidesOnDeactivate = false
        isMovable = false
        animationBehavior = .none
    }

    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }
}

private final class TrayBackgroundView: NSVisualEffectView {
    init() {
        super.init(frame: .zero)
        material = .hudWindow
        blendingMode = .behindWindow
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
