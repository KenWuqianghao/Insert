import AppKit
import SwiftUI

enum RingGeometry {
    static let innerRadius: CGFloat = 72
    static let outerRadius: CGFloat = 132
    static let iconRadius: CGFloat = 102
    static let holeRadius: CGFloat = 62
    static let deadZone = holeRadius
    static let iconSize: CGFloat = 30
    static let selectedIconScale: CGFloat = 1.28
    static let selectionInset: CGFloat = 3
    static let gap: CGFloat = 6
    static let previewGap: CGFloat = 22
    /// Segment 0 is at 12 o'clock. The next segments follow clockwise, because the y axis points down.
    static let firstDirection = -Double.pi / 2

    static func direction(of index: Int, count: Int) -> Double {
        firstDirection + Double(index) * span(count)
    }

    static func span(_ count: Int) -> Double {
        2 * .pi / Double(max(count, 1))
    }

    static func segment(at point: CGPoint, center: CGPoint, count: Int) -> Int? {
        let dx = point.x - center.x
        let dy = point.y - center.y
        guard count > 0, hypot(dx, dy) >= deadZone else { return nil }

        var angle = atan2(dy, dx) - firstDirection
        if angle < 0 {
            angle += 2 * .pi
        }
        return Int((angle / span(count)).rounded()) % count
    }
}

struct RingView: View {
    @ObservedObject var model: RingModel
    let library: ClipLibrary

    var body: some View {
        ZStack {
            DonutView(model: model)
                .id(model.showCount)
                .position(model.center)

            if let index = model.selection, model.clips.indices.contains(index) {
                let clip = model.clips[index]
                ClipCard(
                    clip: clip,
                    thumbnail: clip.kind == .image ? library.thumbnail(for: clip.id) : nil,
                    pinColor: library.pinboards.first { $0.id == clip.pinboardID }?.color.color,
                    isSelected: false,
                    isPrimary: false,
                    isDeleting: false,
                    badge: nil
                )
                .id(clip.id)
                .position(model.previewCenter(for: index))
                .transition(.scale(scale: 0.94).combined(with: .opacity))
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .ignoresSafeArea()
        .animation(.easeOut(duration: 0.12), value: model.selection)
    }
}

private struct DonutView: View {
    @ObservedObject var model: RingModel

    @State private var appeared = false

    var body: some View {
        let side = RingGeometry.outerRadius * 2
        let hairline = Color.white.opacity(0.16)

        ZStack {
            RingGlass()

            Circle().strokeBorder(hairline, lineWidth: 1)
            Circle().stroke(hairline, lineWidth: 1)
                .frame(width: RingGeometry.innerRadius * 2, height: RingGeometry.innerRadius * 2)
            Circle().stroke(hairline, lineWidth: 1)
                .frame(width: RingGeometry.holeRadius * 2, height: RingGeometry.holeRadius * 2)

            if let index = model.selection, model.clips.indices.contains(index) {
                RingSector(
                    direction: RingGeometry.direction(of: index, count: model.clips.count),
                    span: RingGeometry.span(model.clips.count),
                    inner: RingGeometry.innerRadius + RingGeometry.selectionInset,
                    outer: RingGeometry.outerRadius - RingGeometry.selectionInset
                )
                .fill(Color.white.opacity(0.16), style: FillStyle(eoFill: true))
            }

            ForEach(Array(model.clips.enumerated()), id: \.element.id) { index, clip in
                let direction = RingGeometry.direction(of: index, count: model.clips.count)
                RingIcon(clip: clip)
                    .scaleEffect(model.selection == index ? RingGeometry.selectedIconScale : 1)
                    .animation(.spring(response: 0.2, dampingFraction: 0.7), value: model.selection == index)
                    .offset(x: cos(direction) * RingGeometry.iconRadius, y: sin(direction) * RingGeometry.iconRadius)
                    .scaleEffect(appeared ? 1 : 0.4)
                    .opacity(appeared ? 1 : 0)
                    .animation(.spring(response: 0.3, dampingFraction: 0.78).delay(Double(index) * 0.022), value: appeared)
            }

            hole
        }
        .frame(width: side, height: side)
        .opacity(appeared ? 1 : 0)
        .environment(\.colorScheme, .dark)
        .onAppear {
            withAnimation(.easeOut(duration: 0.16)) {
                appeared = true
            }
        }
    }

    @ViewBuilder
    private var hole: some View {
        if let index = model.selection, model.clips.indices.contains(index) {
            let clip = model.clips[index]
            VStack(spacing: 1) {
                Text(clip.customTitle ?? clip.kind.name)
                    .font(.system(size: 12, weight: .semibold))
                Text(clip.createdAt.coarseRelativeDescription(now: Date()))
                    .font(.system(size: 10.5))
                    .foregroundStyle(.secondary)
            }
            .lineLimit(1)
            .frame(width: RingGeometry.holeRadius * 2 - 16)
        }
    }
}

/// A mask image gives the shape, because the blur comes from the window server.
private struct RingGlass: NSViewRepresentable {
    func makeNSView(context: Context) -> NSVisualEffectView {
        let view = PassThroughEffectView()
        view.material = .hudWindow
        view.blendingMode = .behindWindow
        view.state = .active
        view.maskImage = Self.mask
        return view
    }

    func updateNSView(_ nsView: NSVisualEffectView, context: Context) {}

    private static let mask: NSImage = {
        let side = RingGeometry.outerRadius * 2
        return NSImage(size: NSSize(width: side, height: side), flipped: false) { rect in
            let ring = NSBezierPath(ovalIn: rect)
            let band = RingGeometry.outerRadius - RingGeometry.innerRadius
            ring.append(NSBezierPath(ovalIn: rect.insetBy(dx: band, dy: band)))
            ring.windingRule = .evenOdd
            NSColor.black.setFill()
            ring.fill()

            let hole = RingGeometry.outerRadius - RingGeometry.holeRadius
            NSBezierPath(ovalIn: rect.insetBy(dx: hole, dy: hole)).fill()
            return true
        }
    }()
}

private final class PassThroughEffectView: NSVisualEffectView {
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
}

private struct RingIcon: View {
    let clip: Clip

    var body: some View {
        let entry = AppIconStore.shared.entry(for: clip.source)

        Group {
            if let icon = entry.icon {
                Image(nsImage: icon)
                    .resizable()
            } else {
                RoundedRectangle(cornerRadius: 7, style: .continuous)
                    .fill(entry.tint)
            }
        }
        .frame(width: RingGeometry.iconSize, height: RingGeometry.iconSize)
        .overlay(alignment: .bottomTrailing) {
            Image(systemName: clip.kind.symbolName)
                .font(.system(size: 7, weight: .bold))
                .foregroundStyle(.white)
                .frame(width: 14, height: 14)
                .background(Circle().fill(Color.black.opacity(0.6)))
                .overlay(Circle().strokeBorder(Color.white.opacity(0.2), lineWidth: 0.5))
                .offset(x: 4, y: 4)
        }
    }
}

private struct RingSector: Shape {
    let direction: Double
    let span: Double
    let inner: CGFloat
    let outer: CGFloat

    func path(in rect: CGRect) -> Path {
        let center = CGPoint(x: rect.midX, y: rect.midY)
        var path = Path()

        let isFullRing = span >= 2 * .pi - 0.001
        if isFullRing {
            path.addEllipse(in: CGRect(x: center.x - outer, y: center.y - outer, width: outer * 2, height: outer * 2))
            path.addEllipse(in: CGRect(x: center.x - inner, y: center.y - inner, width: inner * 2, height: inner * 2))
            return path
        }

        let halfGapAngle = { (radius: CGFloat) in RingGeometry.gap / 2 / radius }
        path.addArc(
            center: center,
            radius: outer,
            startAngle: .radians(direction - span / 2 + halfGapAngle(outer)),
            endAngle: .radians(direction + span / 2 - halfGapAngle(outer)),
            clockwise: false
        )
        path.addArc(
            center: center,
            radius: inner,
            startAngle: .radians(direction + span / 2 - halfGapAngle(inner)),
            endAngle: .radians(direction - span / 2 + halfGapAngle(inner)),
            clockwise: true
        )
        path.closeSubpath()
        return path
    }
}
