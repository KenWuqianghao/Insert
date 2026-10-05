import SwiftUI

struct ClipPreview: View {
    private enum Loaded {
        case text(String)
        case image(NSImage)
    }

    private static let textLimit = 200_000

    let clip: Clip
    let library: ClipLibrary

    @State private var loaded: Loaded?

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            header

            content
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(Color.cardBackground)
                .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
        }
        .padding(.horizontal, 22)
        .padding(.top, 18)
        .padding(.bottom, 6)
        .task(id: clip.id) {
            loaded = load()
        }
    }

    private var header: some View {
        HStack(spacing: 10) {
            if let icon = AppIconStore.shared.entry(for: clip.source).icon {
                Image(nsImage: icon)
                    .resizable()
                    .frame(width: 28, height: 28)
            }

            VStack(alignment: .leading, spacing: 1) {
                Text(clip.customTitle ?? clip.content.displayTitle)
                    .font(.system(size: 15, weight: .semibold))
                    .lineLimit(1)

                Text([clip.kind.name, clip.source?.name, clip.footerText].compactMap { $0 }.joined(separator: " · "))
                    .font(.system(size: 12))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }

            Spacer()

            Text("Press Space to close")
                .font(.system(size: 11))
                .foregroundStyle(.tertiary)
        }
    }

    @ViewBuilder
    private var content: some View {
        switch clip.content {
        case .text(let snippet), .richText(let snippet):
            ScrollView {
                Text(loadedText ?? snippet.text)
                    .font(.system(size: 13))
                    .lineSpacing(3)
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(16)
            }

        case .image:
            if case .image(let image) = loaded {
                Image(nsImage: image)
                    .resizable()
                    .scaledToFit()
                    .padding(12)
            } else if let thumbnail = library.thumbnail(for: clip.id) {
                Image(nsImage: thumbnail)
                    .resizable()
                    .scaledToFit()
                    .padding(12)
            }

        case .link(let url, let title):
            VStack(spacing: 10) {
                Image(systemName: "link")
                    .font(.system(size: 30, weight: .semibold))
                    .foregroundStyle(Color.accentColor)
                if let title {
                    Text(title)
                        .font(.system(size: 17, weight: .semibold))
                }
                Text(url.absoluteString)
                    .font(.system(size: 14))
                    .foregroundStyle(.secondary)
                    .textSelection(.enabled)
                    .multilineTextAlignment(.center)
            }
            .padding(24)

        case .files(let urls):
            ScrollView {
                VStack(alignment: .leading, spacing: 10) {
                    ForEach(urls, id: \.self) { url in
                        HStack(spacing: 10) {
                            Image(nsImage: AppIconStore.shared.fileIcon(for: url))
                                .resizable()
                                .frame(width: 32, height: 32)
                            VStack(alignment: .leading, spacing: 1) {
                                Text(url.lastPathComponent)
                                    .font(.system(size: 13, weight: .medium))
                                Text(url.deletingLastPathComponent().path)
                                    .font(.system(size: 11))
                                    .foregroundStyle(.secondary)
                            }
                            .lineLimit(1)
                            .truncationMode(.middle)
                        }
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(16)
            }

        case .color(let value):
            value.color.overlay(
                VStack(spacing: 6) {
                    Text(value.hex)
                        .font(.system(size: 28, weight: .semibold, design: .monospaced))
                    Text(clip.footerText)
                        .font(.system(size: 14, weight: .medium, design: .monospaced))
                }
                .foregroundStyle(value.isLight ? Color.black.opacity(0.75) : Color.white)
            )

        case .pdf, .media, .data:
            VStack(spacing: 10) {
                Image(systemName: clip.kind.symbolName)
                    .font(.system(size: 44))
                Text("\(clip.content.displayTitle), \(clip.footerText)")
                    .font(.system(size: 14, weight: .medium))
            }
            .foregroundStyle(.secondary)
        }
    }

    private var loadedText: String? {
        if case .text(let text) = loaded { return text }
        return nil
    }

    private func load() -> Loaded? {
        switch clip.content {
        case .text(let snippet), .richText(let snippet):
            guard snippet.characterCount > TextSnippet.limit, let text = library.payload(for: clip.id)?.plainText else {
                return nil
            }
            return .text(String(text.prefix(Self.textLimit)))
        case .image:
            return library.payload(for: clip.id)?.imageData.flatMap(NSImage.init(data:)).map(Loaded.image)
        case .link, .files, .color, .pdf, .media, .data:
            return nil
        }
    }
}
