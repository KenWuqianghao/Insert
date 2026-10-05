import SwiftUI

struct ClipCard: View {
    let clip: Clip
    let thumbnail: NSImage?
    let pinColor: Color?
    let isSelected: Bool
    let isPrimary: Bool
    let isDeleting: Bool
    let badge: Int?

    private var shape: RoundedRectangle {
        RoundedRectangle(cornerRadius: TrayMetrics.cardCornerRadius, style: .continuous)
    }

    var body: some View {
        let app = AppIconStore.shared.entry(for: clip.source)

        VStack(spacing: 0) {
            header(icon: app.icon)
                .background(app.tint)

            ClipCardBody(content: clip.content, thumbnail: thumbnail)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .clipped()

            footer
        }
        .frame(width: TrayMetrics.cardSize, height: TrayMetrics.cardSize)
        .background(Color.cardBackground)
        .clipShape(shape)
        .overlay(shape.strokeBorder(Color.primary.opacity(0.1), lineWidth: 1))
        .shadow(color: .black.opacity(0.22), radius: 6, y: 3)
        .overlay(
            RoundedRectangle(cornerRadius: TrayMetrics.cardCornerRadius + 4, style: .continuous)
                .strokeBorder(Color.accentColor.opacity(isPrimary ? 1 : 0.55), lineWidth: isPrimary ? 3.5 : 2.5)
                .padding(-5)
                .opacity(isSelected ? 1 : 0)
        )
        .scaleEffect(isDeleting ? 0.94 : 1)
        .offset(x: isDeleting ? -26 : 0, y: isDeleting ? 8 : 0)
        .opacity(isDeleting ? 0 : 1)
        .contentShape(shape)
        .allowsHitTesting(!isDeleting)
        .animation(TrayMetrics.animation, value: isSelected)
        .animation(TrayMetrics.animation, value: isPrimary)
        .animation(TrayMetrics.animation, value: isDeleting)
    }

    private func header(icon: NSImage?) -> some View {
        HStack(spacing: 8) {
            VStack(alignment: .leading, spacing: 2) {
                Text(clip.customTitle ?? clip.kind.name)
                    .font(.system(size: 15, weight: .semibold))
                    .lineLimit(1)

                TimelineView(.periodic(from: .now, by: 60)) { context in
                    Text(subtitle(now: context.date))
                        .font(.system(size: 11, weight: .medium))
                        .opacity(0.82)
                        .lineLimit(1)
                }
            }
            .foregroundStyle(.white)

            Spacer(minLength: 0)

            if let icon {
                Image(nsImage: icon)
                    .resizable()
                    .interpolation(.high)
                    .frame(width: 64, height: 64)
                    .offset(x: 12)
                    .shadow(color: .black.opacity(0.25), radius: 4, y: 1)
            } else {
                Image(systemName: clip.kind.symbolName)
                    .font(.system(size: 22, weight: .semibold))
                    .foregroundStyle(.white.opacity(0.85))
            }
        }
        .padding(.leading, 14)
        .padding(.trailing, icon == nil ? 14 : 0)
        .frame(height: 52)
        .clipped()
    }

    private func subtitle(now: Date) -> String {
        let time = clip.createdAt.coarseRelativeDescription(now: now)
        return clip.customTitle == nil ? time : "\(clip.kind.name) · \(time)"
    }

    private var footer: some View {
        ZStack {
            Text(clip.footerText)
                .font(.system(size: 11, weight: .medium))
                .foregroundStyle(.secondary)
                .lineLimit(1)

            HStack {
                if let pinColor {
                    Circle()
                        .fill(pinColor)
                        .frame(width: 8, height: 8)
                }

                Spacer()

                if let badge {
                    Text("⌘\(badge)")
                        .font(.system(size: 11, weight: .semibold, design: .rounded))
                        .foregroundStyle(.white)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(Color.accentColor, in: Capsule())
                        .transition(.scale(scale: 0.6).combined(with: .opacity))
                }
            }
        }
        .padding(.horizontal, 12)
        .frame(height: 28)
        .background(Color.cardBackground.opacity(0.94))
        .animation(TrayMetrics.animation, value: badge)
    }
}

private struct ClipCardBody: View {
    let content: ClipContent
    let thumbnail: NSImage?

    var body: some View {
        switch content {
        case .text(let snippet), .richText(let snippet):
            Text(snippet.text.prefix(600).trimmingCharacters(in: .whitespacesAndNewlines))
                .font(.system(size: 12.5))
                .lineSpacing(2)
                .foregroundStyle(.primary.opacity(0.88))
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                .padding(.horizontal, 14)
                .padding(.top, 12)

        case .link(let url, let title):
            VStack(alignment: .leading, spacing: 6) {
                Image(systemName: "link")
                    .font(.system(size: 18, weight: .semibold))
                    .foregroundStyle(Color.accentColor)

                Text(title ?? url.host ?? url.absoluteString)
                    .font(.system(size: 15, weight: .semibold))
                    .lineLimit(2)

                Text(url.absoluteString)
                    .font(.system(size: 12))
                    .foregroundStyle(.secondary)
                    .lineLimit(3)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .padding(.horizontal, 14)
            .padding(.top, 14)

        case .image:
            if let thumbnail {
                Color.clear.overlay(
                    Image(nsImage: thumbnail)
                        .resizable()
                        .scaledToFill()
                )
            } else {
                placeholder(symbol: ClipKind.image.symbolName, label: "Image")
            }

        case .color(let value):
            value.color.overlay(
                Text(value.hex)
                    .font(.system(size: 17, weight: .semibold, design: .monospaced))
                    .foregroundStyle(value.isLight ? Color.black.opacity(0.75) : Color.white)
            )

        case .files(let urls):
            VStack(spacing: 8) {
                if let first = urls.first {
                    Image(nsImage: AppIconStore.shared.fileIcon(for: first))
                        .resizable()
                        .frame(width: 56, height: 56)
                }

                VStack(spacing: 2) {
                    ForEach(urls.prefix(3), id: \.self) { url in
                        Text(url.lastPathComponent)
                            .font(.system(size: 12, weight: .medium))
                            .lineLimit(1)
                            .truncationMode(.middle)
                    }
                    if urls.count > 3 {
                        Text("and \(urls.count - 3) more")
                            .font(.system(size: 11))
                            .foregroundStyle(.secondary)
                    }
                }
            }
            .padding(.horizontal, 14)

        case .pdf:
            placeholder(symbol: ClipKind.pdf.symbolName, label: "PDF Document")
        case .media:
            placeholder(symbol: ClipKind.media.symbolName, label: "Media")
        case .data:
            placeholder(symbol: ClipKind.data.symbolName, label: "Clipboard Data")
        }
    }

    private func placeholder(symbol: String, label: String) -> some View {
        VStack(spacing: 8) {
            Image(systemName: symbol)
                .font(.system(size: 36, weight: .regular))
            Text(label)
                .font(.system(size: 12, weight: .medium))
        }
        .foregroundStyle(.secondary)
    }
}
