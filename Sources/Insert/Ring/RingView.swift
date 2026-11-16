import SwiftUI

enum RingGeometry {
    static let innerRadius: CGFloat = 58
    static let outerRadius: CGFloat = 166
    static let labelRadius: CGFloat = 112
    static let bandWidth: CGFloat = 7
    static let gap: CGFloat = 4
    static let lift: CGFloat = 9
    static let deadZone: CGFloat = 30
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
            DonutView(model: model, library: library)
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
    let library: ClipLibrary

    @State private var appeared = false

    var body: some View {
        let side = (RingGeometry.outerRadius + RingGeometry.lift) * 2 + 8

        ZStack {
            ForEach(Array(model.clips.enumerated()), id: \.element.id) { index, clip in
                RingSegment(
                    clip: clip,
                    thumbnail: clip.kind == .image ? library.thumbnail(for: clip.id) : nil,
                    index: index,
                    count: model.clips.count,
                    isSelected: model.selection == index
                )
                .rotationEffect(.degrees(appeared ? 0 : -24))
                .opacity(appeared ? 1 : 0)
                .animation(.spring(response: 0.3, dampingFraction: 0.78).delay(Double(index) * 0.022), value: appeared)
            }

            hole
        }
        .frame(width: side, height: side)
        .compositingGroup()
        .shadow(color: .black.opacity(0.24), radius: 14, y: 6)
        .scaleEffect(appeared ? 1 : 0.6)
        .onAppear {
            withAnimation(.spring(response: 0.26, dampingFraction: 0.8)) {
                appeared = true
            }
        }
    }

    private var hole: some View {
        let selected = model.selection.flatMap { model.clips.indices.contains($0) ? model.clips[$0] : nil }

        return Circle()
            .fill(Color.cardBackground)
            .frame(width: (RingGeometry.innerRadius - 6) * 2, height: (RingGeometry.innerRadius - 6) * 2)
            .overlay {
                if let selected {
                    VStack(spacing: 1) {
                        Text(selected.customTitle ?? selected.kind.name)
                            .font(.system(size: 12, weight: .semibold))
                        Text(selected.createdAt.coarseRelativeDescription(now: Date()))
                            .font(.system(size: 10.5))
                            .foregroundStyle(.secondary)
                    }
                    .lineLimit(1)
                    .padding(.horizontal, 8)
                }
            }
    }
}

private struct RingSegment: View {
    let clip: Clip
    let thumbnail: NSImage?
    let index: Int
    let count: Int
    let isSelected: Bool

    var body: some View {
        let tint = AppIconStore.shared.entry(for: clip.source).tint
        let direction = RingGeometry.direction(of: index, count: count)
        let span = RingGeometry.span(count)
        let lift = isSelected ? RingGeometry.lift : 0
        let fill = FillStyle(eoFill: true)

        ZStack {
            RingSector(direction: direction, span: span, inner: RingGeometry.innerRadius, outer: RingGeometry.outerRadius)
                .fill(Color.cardBackground, style: fill)
            RingSector(direction: direction, span: span, inner: RingGeometry.innerRadius, outer: RingGeometry.outerRadius)
                .fill(tint.opacity(isSelected ? 0.24 : 0), style: fill)
            RingSector(
                direction: direction,
                span: span,
                inner: RingGeometry.outerRadius - RingGeometry.bandWidth,
                outer: RingGeometry.outerRadius
            )
            .fill(tint, style: fill)

            RingSegmentLabel(clip: clip, thumbnail: thumbnail)
                .offset(x: cos(direction) * RingGeometry.labelRadius, y: sin(direction) * RingGeometry.labelRadius)
        }
        .offset(x: cos(direction) * lift, y: sin(direction) * lift)
        .animation(.spring(response: 0.2, dampingFraction: 0.7), value: isSelected)
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
        let outerInset = halfGapAngle(outer)
        let innerInset = halfGapAngle(inner)
        path.addArc(
            center: center,
            radius: outer,
            startAngle: .radians(direction - span / 2 + outerInset),
            endAngle: .radians(direction + span / 2 - outerInset),
            clockwise: false
        )
        path.addArc(
            center: center,
            radius: inner,
            startAngle: .radians(direction + span / 2 - innerInset),
            endAngle: .radians(direction - span / 2 + innerInset),
            clockwise: true
        )
        path.closeSubpath()
        return path
    }
}

private struct RingSegmentLabel: View {
    let clip: Clip
    let thumbnail: NSImage?

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(clip.customTitle ?? clip.kind.name)
                .font(.system(size: 9, weight: .bold))
                .textCase(.uppercase)
                .foregroundStyle(.secondary)
                .lineLimit(1)

            content
        }
        .frame(width: 70, height: 56, alignment: .topLeading)
    }

    @ViewBuilder
    private var content: some View {
        switch clip.content {
        case .text(let snippet), .richText(let snippet):
            Text(snippet.text.prefix(120).trimmingCharacters(in: .whitespacesAndNewlines))
                .font(.system(size: 10))
                .lineLimit(3)

        case .link(let url, let title):
            Text(title ?? url.host ?? url.absoluteString)
                .font(.system(size: 10, weight: .semibold))
                .lineLimit(2)

        case .image:
            if let thumbnail {
                Color.clear
                    .frame(width: 70, height: 38)
                    .overlay(Image(nsImage: thumbnail).resizable().scaledToFill())
                    .clipShape(RoundedRectangle(cornerRadius: 5, style: .continuous))
            } else {
                symbol
            }

        case .color(let value):
            RoundedRectangle(cornerRadius: 5, style: .continuous)
                .fill(value.color)
                .frame(width: 70, height: 30)
                .overlay(
                    Text(value.hex)
                        .font(.system(size: 9, weight: .semibold, design: .monospaced))
                        .foregroundStyle(value.isLight ? Color.black.opacity(0.75) : Color.white)
                )

        case .files(let urls):
            HStack(spacing: 5) {
                if let first = urls.first {
                    Image(nsImage: AppIconStore.shared.fileIcon(for: first))
                        .resizable()
                        .frame(width: 18, height: 18)
                }
                Text(clip.content.displayTitle)
                    .font(.system(size: 10))
                    .lineLimit(2)
                    .truncationMode(.middle)
            }

        case .pdf, .media, .data:
            symbol
        }
    }

    private var symbol: some View {
        Image(systemName: clip.kind.symbolName)
            .font(.system(size: 20))
            .foregroundStyle(.secondary)
    }
}
