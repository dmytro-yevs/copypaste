import Cocoa

struct QuickPasteDisplay {
  let frame: NSRect
  let visibleFrame: NSRect
}

func quickPasteInspectorFrame(current: NSRect, visible: NSRect, expanded: Bool) -> NSRect {
  let width = min(expanded ? 816 : 448, visible.width)
  let height = min(CGFloat(800), visible.height)
  let x = min(max(current.minX, visible.minX), max(visible.minX, visible.maxX - width))
  let y = min(max(current.maxY - height, visible.minY), max(visible.minY, visible.maxY - height))
  return NSRect(x: x, y: y, width: width, height: height)
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
