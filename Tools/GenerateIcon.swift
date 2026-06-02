import AppKit
import Foundation

let root = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
let resourcesURL = root.appendingPathComponent("Resources", isDirectory: true)
let iconsetURL = resourcesURL.appendingPathComponent("AppIcon.iconset", isDirectory: true)
let outputURL = resourcesURL.appendingPathComponent("AppIcon.icns")

try FileManager.default.createDirectory(at: iconsetURL, withIntermediateDirectories: true)

func makeBaseIcon() -> NSImage {
    let size = NSSize(width: 1024, height: 1024)
    let image = NSImage(size: size)

    image.lockFocus()
    defer { image.unlockFocus() }

    NSColor.clear.setFill()
    NSRect(origin: .zero, size: size).fill()

    let background = NSBezierPath(roundedRect: NSRect(x: 64, y: 64, width: 896, height: 896), xRadius: 210, yRadius: 210)
    let gradient = NSGradient(colors: [
        NSColor(calibratedRed: 0.12, green: 0.13, blue: 0.15, alpha: 1),
        NSColor(calibratedRed: 0.03, green: 0.04, blue: 0.05, alpha: 1)
    ])
    gradient?.draw(in: background, angle: -35)

    NSColor(calibratedWhite: 1, alpha: 0.10).setStroke()
    background.lineWidth = 8
    background.stroke()

    let trayRect = NSRect(x: 166, y: 244, width: 692, height: 404)
    let trayPath = NSBezierPath(roundedRect: trayRect, xRadius: 52, yRadius: 52)
    NSColor(calibratedWhite: 1, alpha: 0.14).setFill()
    trayPath.fill()
    NSColor(calibratedWhite: 1, alpha: 0.24).setStroke()
    trayPath.lineWidth = 6
    trayPath.stroke()

    let searchRect = NSRect(x: 218, y: 560, width: 588, height: 62)
    let searchPath = NSBezierPath(roundedRect: searchRect, xRadius: 26, yRadius: 26)
    NSColor(calibratedWhite: 1, alpha: 0.16).setFill()
    searchPath.fill()

    drawMagnifier(center: NSPoint(x: 258, y: 591), radius: 13, lineWidth: 7, color: NSColor(calibratedWhite: 1, alpha: 0.72))

    let cardColors: [NSColor] = [
        NSColor(calibratedRed: 0.96, green: 0.96, blue: 0.92, alpha: 1),
        NSColor(calibratedRed: 0.79, green: 0.89, blue: 0.96, alpha: 1),
        NSColor(calibratedRed: 0.89, green: 0.84, blue: 0.98, alpha: 1)
    ]

    for index in 0..<3 {
        let x = 226 + CGFloat(index) * 194
        let cardRect = NSRect(x: x, y: 316, width: 158, height: 184)
        let cardPath = NSBezierPath(roundedRect: cardRect, xRadius: 24, yRadius: 24)
        cardColors[index].setFill()
        cardPath.fill()

        NSColor(calibratedWhite: 0, alpha: 0.12).setStroke()
        cardPath.lineWidth = 4
        cardPath.stroke()

        NSColor(calibratedWhite: 0.10, alpha: 0.58).setFill()
        NSBezierPath(roundedRect: NSRect(x: x + 24, y: 444, width: 78, height: 12), xRadius: 6, yRadius: 6).fill()
        NSBezierPath(roundedRect: NSRect(x: x + 24, y: 410, width: 110, height: 10), xRadius: 5, yRadius: 5).fill()
        NSBezierPath(roundedRect: NSRect(x: x + 24, y: 382, width: 92, height: 10), xRadius: 5, yRadius: 5).fill()
    }

    let liftPath = NSBezierPath()
    liftPath.move(to: NSPoint(x: 378, y: 232))
    liftPath.line(to: NSPoint(x: 512, y: 164))
    liftPath.line(to: NSPoint(x: 646, y: 232))
    NSColor(calibratedWhite: 1, alpha: 0.82).setStroke()
    liftPath.lineWidth = 24
    liftPath.lineCapStyle = .round
    liftPath.lineJoinStyle = .round
    liftPath.stroke()

    return image
}

func drawMagnifier(center: NSPoint, radius: CGFloat, lineWidth: CGFloat, color: NSColor) {
    color.setStroke()
    let circle = NSBezierPath(ovalIn: NSRect(x: center.x - radius, y: center.y - radius, width: radius * 2, height: radius * 2))
    circle.lineWidth = lineWidth
    circle.stroke()

    let handle = NSBezierPath()
    handle.move(to: NSPoint(x: center.x + radius * 0.68, y: center.y - radius * 0.68))
    handle.line(to: NSPoint(x: center.x + radius * 1.62, y: center.y - radius * 1.62))
    handle.lineWidth = lineWidth
    handle.lineCapStyle = .round
    handle.stroke()
}

func writePNG(_ image: NSImage, pixels: Int, to url: URL) throws {
    let outputSize = NSSize(width: pixels, height: pixels)
    guard
        let bitmap = NSBitmapImageRep(
            bitmapDataPlanes: nil,
            pixelsWide: pixels,
            pixelsHigh: pixels,
            bitsPerSample: 8,
            samplesPerPixel: 4,
            hasAlpha: true,
            isPlanar: false,
            colorSpaceName: .deviceRGB,
            bytesPerRow: 0,
            bitsPerPixel: 0
        )
    else {
        throw NSError(domain: "InsertIcon", code: 1, userInfo: [NSLocalizedDescriptionKey: "Failed to create bitmap"])
    }

    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)
    NSGraphicsContext.current?.imageInterpolation = .high
    image.draw(in: NSRect(origin: .zero, size: outputSize), from: NSRect(origin: .zero, size: image.size), operation: .copy, fraction: 1)
    NSGraphicsContext.restoreGraphicsState()

    guard
        let png = bitmap.representation(using: .png, properties: [:])
    else {
        throw NSError(domain: "InsertIcon", code: 1, userInfo: [NSLocalizedDescriptionKey: "Failed to encode PNG"])
    }

    try png.write(to: url)
}

let icon = makeBaseIcon()
let renditions: [(String, Int)] = [
    ("icon_16x16.png", 16),
    ("icon_16x16@2x.png", 32),
    ("icon_32x32.png", 32),
    ("icon_32x32@2x.png", 64),
    ("icon_128x128.png", 128),
    ("icon_128x128@2x.png", 256),
    ("icon_256x256.png", 256),
    ("icon_256x256@2x.png", 512),
    ("icon_512x512.png", 512),
    ("icon_512x512@2x.png", 1024)
]

for rendition in renditions {
    try writePNG(icon, pixels: rendition.1, to: iconsetURL.appendingPathComponent(rendition.0))
}

if FileManager.default.fileExists(atPath: outputURL.path) {
    try FileManager.default.removeItem(at: outputURL)
}

let process = Process()
process.executableURL = URL(fileURLWithPath: "/usr/bin/iconutil")
process.arguments = ["-c", "icns", iconsetURL.path, "-o", outputURL.path]
try process.run()
process.waitUntilExit()

guard process.terminationStatus == 0 else {
    throw NSError(domain: "InsertIcon", code: Int(process.terminationStatus), userInfo: [NSLocalizedDescriptionKey: "iconutil failed"])
}

print(outputURL.path)
