import AppKit
import Foundation

let root = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
let assetsURL = root.appendingPathComponent("docs/assets", isDirectory: true)
try FileManager.default.createDirectory(at: assetsURL, withIntermediateDirectories: true)

struct Palette {
    static let ink = NSColor(calibratedRed: 0.07, green: 0.08, blue: 0.09, alpha: 1)
    static let panel = NSColor(calibratedRed: 0.12, green: 0.13, blue: 0.15, alpha: 1)
    static let panel2 = NSColor(calibratedRed: 0.17, green: 0.18, blue: 0.20, alpha: 1)
    static let white = NSColor(calibratedWhite: 1, alpha: 1)
    static let muted = NSColor(calibratedWhite: 1, alpha: 0.58)
    static let line = NSColor(calibratedWhite: 1, alpha: 0.12)
    static let mint = NSColor(calibratedRed: 0.58, green: 0.86, blue: 0.70, alpha: 1)
    static let blue = NSColor(calibratedRed: 0.55, green: 0.75, blue: 0.95, alpha: 1)
    static let coral = NSColor(calibratedRed: 0.97, green: 0.61, blue: 0.54, alpha: 1)
    static let gold = NSColor(calibratedRed: 0.95, green: 0.80, blue: 0.48, alpha: 1)
}

func rounded(_ rect: NSRect, radius: CGFloat) -> NSBezierPath {
    NSBezierPath(roundedRect: rect, xRadius: radius, yRadius: radius)
}

func fill(_ rect: NSRect, radius: CGFloat, color: NSColor) {
    color.setFill()
    rounded(rect, radius: radius).fill()
}

func text(_ value: String, at point: NSPoint, size: CGFloat, weight: NSFont.Weight, color: NSColor, width: CGFloat = 400) {
    let paragraph = NSMutableParagraphStyle()
    paragraph.lineBreakMode = .byTruncatingTail
    let attrs: [NSAttributedString.Key: Any] = [
        .font: NSFont.systemFont(ofSize: size, weight: weight),
        .foregroundColor: color,
        .paragraphStyle: paragraph
    ]
    value.draw(in: NSRect(x: point.x, y: point.y, width: width, height: size * 1.45), withAttributes: attrs)
}

func drawSearch(in rect: NSRect) {
    fill(rect, radius: 18, color: NSColor(calibratedWhite: 1, alpha: 0.08))
    NSColor(calibratedWhite: 1, alpha: 0.16).setStroke()
    rounded(rect, radius: 18).stroke()

    NSColor(calibratedWhite: 1, alpha: 0.72).setStroke()
    let lens = NSBezierPath(ovalIn: NSRect(x: rect.minX + 18, y: rect.midY - 6, width: 12, height: 12))
    lens.lineWidth = 2
    lens.stroke()
    let handle = NSBezierPath()
    handle.move(to: NSPoint(x: rect.minX + 29, y: rect.midY - 8))
    handle.line(to: NSPoint(x: rect.minX + 37, y: rect.midY - 16))
    handle.lineWidth = 2
    handle.lineCapStyle = .round
    handle.stroke()

    text("Search clips, links, images, files...", at: NSPoint(x: rect.minX + 52, y: rect.minY + 12), size: 16, weight: .regular, color: Palette.muted, width: rect.width - 70)
}

func drawCard(_ rect: NSRect, title: String, detail: String, badge: String, color: NSColor, selected: Bool = false) {
    let path = rounded(rect, radius: 18)
    color.setFill()
    path.fill()

    (selected ? Palette.white : NSColor(calibratedWhite: 0, alpha: 0.10)).setStroke()
    path.lineWidth = selected ? 3 : 1
    path.stroke()

    let badgeRect = NSRect(x: rect.minX + 18, y: rect.maxY - 44, width: 42, height: 26)
    fill(badgeRect, radius: 13, color: NSColor(calibratedWhite: 0, alpha: 0.10))
    text(badge, at: NSPoint(x: badgeRect.minX + 10, y: badgeRect.minY + 4), size: 12, weight: .semibold, color: NSColor(calibratedWhite: 0.10, alpha: 0.78), width: 26)

    text(title, at: NSPoint(x: rect.minX + 18, y: rect.maxY - 82), size: 18, weight: .semibold, color: Palette.ink, width: rect.width - 36)
    text(detail, at: NSPoint(x: rect.minX + 18, y: rect.maxY - 112), size: 13, weight: .regular, color: NSColor(calibratedWhite: 0.12, alpha: 0.66), width: rect.width - 36)

    NSColor(calibratedWhite: 0, alpha: 0.16).setFill()
    rounded(NSRect(x: rect.minX + 18, y: rect.minY + 22, width: rect.width * 0.62, height: 7), radius: 4).fill()
    rounded(NSRect(x: rect.minX + 18, y: rect.minY + 40, width: rect.width * 0.78, height: 7), radius: 4).fill()
}

func writePNG(_ image: NSImage, to url: URL) throws {
    guard let tiff = image.tiffRepresentation, let rep = NSBitmapImageRep(data: tiff), let data = rep.representation(using: .png, properties: [:]) else {
        throw NSError(domain: "InsertMarketingAssets", code: 1, userInfo: [NSLocalizedDescriptionKey: "Failed to encode \(url.lastPathComponent)"])
    }
    try data.write(to: url)
}

func makeTrayScreenshot() -> NSImage {
    let size = NSSize(width: 1600, height: 1000)
    let image = NSImage(size: size)
    image.lockFocus()
    defer { image.unlockFocus() }

    NSGradient(colors: [
        NSColor(calibratedRed: 0.94, green: 0.95, blue: 0.96, alpha: 1),
        NSColor(calibratedRed: 0.80, green: 0.84, blue: 0.87, alpha: 1)
    ])?.draw(in: NSRect(origin: .zero, size: size), angle: 90)

    let desktopWindow = NSRect(x: 170, y: 210, width: 1260, height: 620)
    fill(desktopWindow, radius: 18, color: NSColor(calibratedWhite: 1, alpha: 0.42))
    NSColor(calibratedWhite: 0, alpha: 0.08).setStroke()
    rounded(desktopWindow, radius: 18).stroke()

    text("Insert", at: NSPoint(x: 230, y: 760), size: 28, weight: .bold, color: NSColor(calibratedWhite: 0, alpha: 0.42), width: 200)
    text("A quiet clipboard tray for macOS", at: NSPoint(x: 230, y: 720), size: 18, weight: .medium, color: NSColor(calibratedWhite: 0, alpha: 0.32), width: 360)

    let tray = NSRect(x: 120, y: 0, width: 1360, height: 365)
    fill(tray, radius: 34, color: Palette.panel)
    NSColor(calibratedWhite: 1, alpha: 0.16).setStroke()
    rounded(tray, radius: 34).stroke()

    drawSearch(in: NSRect(x: 185, y: 280, width: 1230, height: 54))

    drawCard(NSRect(x: 185, y: 64, width: 250, height: 185), title: "Launch notes", detail: "Markdown copied 2m ago", badge: "TXT", color: Palette.mint, selected: true)
    drawCard(NSRect(x: 465, y: 64, width: 250, height: 185), title: "hero-preview.png", detail: "Image copied 8m ago", badge: "IMG", color: Palette.blue)
    drawCard(NSRect(x: 745, y: 64, width: 250, height: 185), title: "Invoice.pdf", detail: "PDF copied 18m ago", badge: "PDF", color: Palette.coral)
    drawCard(NSRect(x: 1025, y: 64, width: 250, height: 185), title: "brand colors", detail: "Rich text copied today", badge: "RTF", color: Palette.gold)

    text("⌘C or Enter to copy • ← → to navigate • Delete removes", at: NSPoint(x: 185, y: 26), size: 14, weight: .medium, color: NSColor(calibratedWhite: 1, alpha: 0.52), width: 620)
    return image
}

func makeHeroScreenshot() -> NSImage {
    let size = NSSize(width: 1400, height: 900)
    let image = NSImage(size: size)
    image.lockFocus()
    defer { image.unlockFocus() }

    NSColor(calibratedRed: 0.09, green: 0.10, blue: 0.11, alpha: 1).setFill()
    NSRect(origin: .zero, size: size).fill()

    let titleRect = NSRect(x: 80, y: 650, width: 520, height: 90)
    text("Insert", at: titleRect.origin, size: 68, weight: .bold, color: Palette.white, width: titleRect.width)
    text("A native clipboard tray that lifts from the bottom of your screen.", at: NSPoint(x: 86, y: 600), size: 23, weight: .medium, color: Palette.muted, width: 560)

    let tray = NSRect(x: 90, y: 90, width: 1220, height: 380)
    fill(tray, radius: 32, color: Palette.panel2)
    NSColor(calibratedWhite: 1, alpha: 0.13).setStroke()
    rounded(tray, radius: 32).stroke()

    drawSearch(in: NSRect(x: 145, y: 390, width: 1110, height: 54))
    drawCard(NSRect(x: 145, y: 160, width: 245, height: 190), title: "latest snippet", detail: "Selected automatically", badge: "TXT", color: Palette.mint, selected: true)
    drawCard(NSRect(x: 420, y: 160, width: 245, height: 190), title: "screenshot.png", detail: "Images and files", badge: "IMG", color: Palette.blue)
    drawCard(NSRect(x: 695, y: 160, width: 245, height: 190), title: "proposal.pdf", detail: "Documents", badge: "PDF", color: Palette.coral)
    drawCard(NSRect(x: 970, y: 160, width: 245, height: 190), title: "meeting link", detail: "URLs and HTML", badge: "URL", color: Palette.gold)

    return image
}

try writePNG(makeTrayScreenshot(), to: assetsURL.appendingPathComponent("insert-tray.png"))
try writePNG(makeHeroScreenshot(), to: assetsURL.appendingPathComponent("insert-hero.png"))

let iconSource = root.appendingPathComponent("Resources/AppIcon.iconset/icon_512x512@2x.png")
let iconDestination = assetsURL.appendingPathComponent("insert-icon.png")
if FileManager.default.fileExists(atPath: iconSource.path) {
    try? FileManager.default.removeItem(at: iconDestination)
    try FileManager.default.copyItem(at: iconSource, to: iconDestination)
}

print(assetsURL.path)
