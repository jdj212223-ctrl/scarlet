// Draws Scarlet's app icon (dark tile, scarlet ring, white play triangle) at every size macOS wants, into Scarlet.iconset.
import AppKit
let out = CommandLine.arguments[1]
try? FileManager.default.createDirectory(atPath: out, withIntermediateDirectories: true)
func draw(_ px: Int) -> Data {
    let s = CGFloat(px)
    let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: px, pixelsHigh: px, bitsPerSample: 8, samplesPerPixel: 4,
                               hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    // macOS icon grid: the shape fills ~80% of the canvas with a soft shadow around it.
    let inset = s * 0.1, box = NSRect(x: inset, y: inset * 1.15, width: s - inset * 2, height: s - inset * 2)
    let shadow = NSShadow(); shadow.shadowBlurRadius = s * 0.025; shadow.shadowOffset = NSSize(width: 0, height: -s * 0.012)
    shadow.shadowColor = NSColor.black.withAlphaComponent(0.35); shadow.set()
    let shape = NSBezierPath(roundedRect: box, xRadius: box.width * 0.225, yRadius: box.width * 0.225)
    // Our own look (not YouTube's red box): a dark tile with a scarlet ring around a white play triangle.
    NSGradient(starting: NSColor(white: 0.17, alpha: 1), ending: NSColor(white: 0.05, alpha: 1))!.draw(in: shape, angle: -90)
    NSShadow().set()
    let ringR = box.width * 0.33
    let ring = NSBezierPath(ovalIn: NSRect(x: box.midX - ringR, y: box.midY - ringR, width: ringR * 2, height: ringR * 2))
    ring.lineWidth = box.width * 0.07
    NSColor(red: 0.93, green: 0.11, blue: 0.16, alpha: 1).setStroke(); ring.stroke()
    // Play triangle, nudged right so it looks centred.
    let c = NSPoint(x: box.midX + box.width * 0.03, y: box.midY), r = box.width * 0.15
    let tri = NSBezierPath()
    tri.move(to: NSPoint(x: c.x - r * 0.8, y: c.y + r)); tri.line(to: NSPoint(x: c.x + r, y: c.y)); tri.line(to: NSPoint(x: c.x - r * 0.8, y: c.y - r)); tri.close()
    tri.lineJoinStyle = .round; tri.lineWidth = r * 0.3
    NSColor.white.setFill(); NSColor.white.setStroke(); tri.fill(); tri.stroke()
    NSGraphicsContext.restoreGraphicsState()
    return rep.representation(using: .png, properties: [:])!
}
for (name, px) in [("16x16", 16), ("16x16@2x", 32), ("32x32", 32), ("32x32@2x", 64), ("128x128", 128), ("128x128@2x", 256),
                   ("256x256", 256), ("256x256@2x", 512), ("512x512", 512), ("512x512@2x", 1024)] {
    try! draw(px).write(to: URL(fileURLWithPath: "\(out)/icon_\(name).png"))
}
