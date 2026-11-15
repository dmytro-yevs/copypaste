import Cocoa
import XCTest
@testable import CopyPaste_Dev

final class WindowMaterialTests: XCTestCase {
  func testBackdropSitsBelowContentAndNeverTakesClicks() {
    let host = NSView(frame: NSRect(x: 0, y: 0, width: 200, height: 100))
    let content = NSView(frame: host.bounds)
    host.addSubview(content)

    let backdrop = WindowMaterialBackdropView()
    backdrop.install(in: host)

    XCTAssertTrue(host.subviews.first === backdrop)
    XCTAssertEqual(backdrop.frame, host.bounds)
    XCTAssertTrue(host.hitTest(NSPoint(x: 100, y: 50)) === content)

    content.removeFromSuperview()
    XCTAssertTrue(host.hitTest(NSPoint(x: 100, y: 50)) === host)
  }
}
