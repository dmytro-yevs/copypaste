import Cocoa
import FlutterMacOS
import XCTest
@testable import CopyPaste_Dev

class RunnerTests: XCTestCase {

  func testTrafficLightButtonsCenterInTheUnifiedTitlebar() {
    XCTAssertEqual(
      MainFlutterWindow.trafficLightButtonOriginY(
        headerHeight: 48,
        buttonHeight: 14
      ),
      17
    )
  }

  func testUnifiedTitlebarContainerPreservesItsTopEdge() {
    let frame = NSRect(x: 12, y: 20, width: 320, height: 28)

    let resized = MainFlutterWindow.unifiedTitlebarContainerFrame(
      frame,
      headerHeight: 48
    )

    XCTAssertEqual(resized.minX, frame.minX)
    XCTAssertEqual(resized.maxY, frame.maxY)
    XCTAssertEqual(resized.width, frame.width)
    XCTAssertEqual(resized.height, 48)

    let oversized = MainFlutterWindow.unifiedTitlebarContainerFrame(
      NSRect(x: 12, y: 20, width: 320, height: 64),
      headerHeight: 48
    )
    XCTAssertEqual(oversized.maxY, 84)
    XCTAssertEqual(oversized.height, 48)
  }

  func testTitlebarViewFillsTheUnifiedTitlebarContainer() {
    let frame = MainFlutterWindow.unifiedTitlebarViewFrame(
      containerBounds: NSRect(x: 0, y: 0, width: 320, height: 48),
      headerHeight: 48
    )

    XCTAssertEqual(frame.origin, .zero)
    XCTAssertEqual(frame.width, 320)
    XCTAssertEqual(frame.height, 48)
  }

  func testTrafficLightLayoutIsDisabledOutsideTheUnifiedNonFullscreenWindow() {
    XCTAssertTrue(
      MainFlutterWindow.usesUnifiedTrafficLightLayout(
        styleMask: [.fullSizeContentView]
      )
    )
    XCTAssertFalse(
      MainFlutterWindow.usesUnifiedTrafficLightLayout(
        styleMask: [.fullSizeContentView, .fullScreen]
      )
    )
  }

  func testQuickPasteUsesTheDisplayContainingTheCursor() {
    let left = QuickPasteDisplay(
      frame: NSRect(x: -1920, y: 0, width: 1920, height: 1080),
      visibleFrame: NSRect(x: -1920, y: 24, width: 1920, height: 1056)
    )
    let primary = QuickPasteDisplay(
      frame: NSRect(x: 0, y: 0, width: 2560, height: 1440),
      visibleFrame: NSRect(x: 0, y: 40, width: 2560, height: 1400)
    )

    let frame = quickPasteFrame(
      cursor: NSPoint(x: -1700, y: 900),
      size: NSSize(width: 520, height: 720),
      displays: [primary, left]
    )

    XCTAssertEqual(frame.origin.x, -1700)
    XCTAssertEqual(frame.maxY, 892)
    XCTAssertTrue(left.visibleFrame.contains(frame))
  }

  func testQuickPasteClampsToTheCursorDisplayWorkArea() {
    let display = QuickPasteDisplay(
      frame: NSRect(x: 2560, y: -200, width: 1600, height: 1000),
      visibleFrame: NSRect(x: 2560, y: -160, width: 1600, height: 940)
    )

    let frame = quickPasteFrame(
      cursor: NSPoint(x: 4140, y: -130),
      size: NSSize(width: 520, height: 720),
      displays: [display]
    )

    XCTAssertEqual(frame.maxX, display.visibleFrame.maxX)
    XCTAssertEqual(frame.minY, display.visibleFrame.minY)
    XCTAssertTrue(display.visibleFrame.contains(frame))
  }

}
