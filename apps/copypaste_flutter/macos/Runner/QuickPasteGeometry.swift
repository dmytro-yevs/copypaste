import Cocoa

struct QuickPasteDisplay {
  let frame: NSRect
  let visibleFrame: NSRect
}

func quickPasteFrame(
  cursor: NSPoint,
  size: NSSize,
  displays: [QuickPasteDisplay],
  gap: CGFloat = 8
) -> NSRect {
  guard let display = displays.first(where: { $0.frame.contains(cursor) }) ?? displays.first else {
    return NSRect(origin: cursor, size: size)
  }
  let visible = display.visibleFrame
  let width = min(size.width, visible.width)
  let height = min(size.height, visible.height)
  let maximumX = max(visible.minX, visible.maxX - width)
  let x = min(max(cursor.x, visible.minX), maximumX)
  let requestedTop = cursor.y - gap
  let maximumY = max(visible.minY, visible.maxY - height)
  let y = min(max(requestedTop - height, visible.minY), maximumY)
  return NSRect(x: x, y: y, width: width, height: height)
}
