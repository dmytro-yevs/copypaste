import Cocoa
import FlutterMacOS
import XCTest
@testable import CopyPaste_Dev

class RunnerTests: XCTestCase {

  func testTerminationDoesNotRequireAFlutterWindowOrReply() {
    let delegate = AppDelegate()
    delegate.mainFlutterWindow = nil
    XCTAssertEqual(delegate.applicationShouldTerminate(NSApplication.shared), .terminateNow)
    XCTAssertEqual(delegate.applicationShouldTerminate(NSApplication.shared), .terminateNow)
    XCTAssertFalse(delegate.applicationShouldTerminateAfterLastWindowClosed(NSApplication.shared))
  }

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

  func testQuickPastePanelDoesNotActivateTheMainApplication() {
    XCTAssertTrue(
      QuickPastePanel.presentationStyleMask.contains(.nonactivatingPanel)
    )
  }

  func testQuickPasteRetainsItsSizeWhenInstallingAZeroSizedFlutterView() {
    let size = NSSize(width: 520, height: 720)
    let panel = QuickPastePanel(
      contentRect: NSRect(origin: .zero, size: size),
      styleMask: QuickPastePanel.presentationStyleMask,
      backing: .buffered,
      defer: false
    )
    panel.isReleasedWhenClosed = false
    defer { panel.close() }
    let controller = NSViewController()
    controller.view = NSView(frame: .zero)

    panel.contentViewController = controller

    XCTAssertEqual(panel.frame.size, size)
    XCTAssertEqual(controller.view.frame.size, size)
  }

  func testQuickPasteInspectorClampsToTheDisplayWorkArea() {
    let visible = NSRect(x: -1920, y: 24, width: 1920, height: 1032)
    let current = NSRect(x: -300, y: 200, width: 448, height: 800)
    let expanded = quickPasteInspectorFrame(current: current, visible: visible, expanded: true)
    XCTAssertEqual(expanded.width, 816)
    XCTAssertEqual(expanded.maxY, current.maxY)
    XCTAssertTrue(visible.contains(expanded))
    let small = NSRect(x: 0, y: 24, width: 640, height: 456)
    let fitted = quickPasteInspectorFrame(current: current, visible: small, expanded: true)
    XCTAssertEqual(fitted.size, small.size)
    XCTAssertTrue(small.contains(fitted))
  }

}

private final class FakeQuickPasteTarget: QuickPasteApplicationTarget {
  let processIdentifier: pid_t
  var isTerminated = false
  var activationSucceeds = true
  var activations = 0
  var onActivate: (() -> Void)?
  init(_ pid: pid_t) { processIdentifier = pid }
  func activateForQuickPaste() -> Bool {
    activations += 1
    onActivate?()
    return activationSucceeds
  }
}

final class QuickPastePasteSessionTests: XCTestCase {
  private var foreground: pid_t? = 2
  private var trusted = true
  private var constructionSucceeds = true
  private var submissions = 0
  private var hidden = 0
  private var cancellations = 0
  private var continuation: (() -> Void)?
  private var results: [Bool] = []

  private func session() -> QuickPastePasteSession {
    QuickPastePasteSession(
      ownPID: 1,
      foreground: { self.foreground },
      trusted: { self.trusted },
      prepareInput: {
        self.constructionSucceeds ? { self.submissions += 1 } : nil
      },
      schedule: { callback in
        self.continuation = callback
        return { self.cancellations += 1 }
      }
    )
  }

  private func paste(_ session: QuickPastePasteSession, _ id: Int64) {
    session.paste(id: id, hide: { self.hidden += 1 }, result: { self.results.append($0) })
  }

  func testDismissThenOwnOrMissingForegroundDoesNotReuseExternalTarget() {
    for frontmost in [FakeQuickPasteTarget(1), nil] {
      let owner = session()
      _ = owner.begin(frontmost: FakeQuickPasteTarget(2), popupActive: false)
      owner.invalidate()
      let id = owner.begin(frontmost: frontmost, popupActive: false)!
      paste(owner, id)
      XCTAssertNil(owner.target)
    }
    XCTAssertEqual(results, [false, false])
    XCTAssertEqual(submissions, 0)
    XCTAssertEqual(hidden, 0)
  }

  func testActivePopupReopenPreservesTargetWithFreshIdentity() {
    let owner = session()
    let target = FakeQuickPasteTarget(2)
    let oldID = owner.begin(frontmost: target, popupActive: false)!
    let id = owner.begin(frontmost: FakeQuickPasteTarget(1), popupActive: true)!
    XCTAssertGreaterThan(id, oldID)
    XCTAssertTrue(owner.target === target)
    paste(owner, oldID)
    paste(owner, id)
    continuation?()
    continuation?()
    XCTAssertEqual(results, [false, true])
    XCTAssertEqual(submissions, 1)
    XCTAssertEqual(target.activations, 0)
    paste(owner, id)
    XCTAssertEqual(results, [false, true, false])
  }

  func testNewExternalForegroundReplacesTargetEvenIfPopupWasActive() {
    let owner = session()
    _ = owner.begin(frontmost: FakeQuickPasteTarget(2), popupActive: false)
    let replacement = FakeQuickPasteTarget(3)
    _ = owner.begin(frontmost: replacement, popupActive: true)
    XCTAssertTrue(owner.target === replacement)
  }

  func testDeadTargetAndActivationFailureSubmitNothing() {
    let owner = session()
    let target = FakeQuickPasteTarget(2)
    let id = owner.begin(frontmost: target, popupActive: false)!
    target.isTerminated = true
    paste(owner, id)
    XCTAssertEqual(target.activations, 0)
    target.isTerminated = false
    foreground = 3
    target.activationSucceeds = false
    let next = owner.begin(frontmost: target, popupActive: false)!
    paste(owner, next)
    XCTAssertEqual(target.activations, 1)
    XCTAssertNil(continuation)
    XCTAssertEqual(results, [false, false])
    XCTAssertEqual(submissions, 0)
  }

  func testActivationSuccessStillRequiresFinalExactForeground() {
    let owner = session()
    let target = FakeQuickPasteTarget(2)
    foreground = 3
    let id = owner.begin(frontmost: target, popupActive: false)!
    paste(owner, id)
    XCTAssertEqual(target.activations, 1)
    continuation?()
    XCTAssertEqual(results, [false])
    XCTAssertEqual(submissions, 0)
  }

  func testFocusTargetTrustAndConstructionAreRecheckedAfterHandoff() {
    for change in [0, 1, 2, 3] {
      foreground = 2; trusted = true; constructionSucceeds = true
      let owner = session()
      let target = FakeQuickPasteTarget(2)
      let id = owner.begin(frontmost: target, popupActive: false)!
      paste(owner, id)
      switch change {
      case 0: foreground = 3
      case 1: target.isTerminated = true
      case 2: trusted = false
      default: constructionSucceeds = false
      }
      continuation?()
      continuation?()
    }
    XCTAssertEqual(results, [false, false, false, false])
    XCTAssertEqual(submissions, 0)
  }

  func testInvalidationCancelsAndCompletesOnceEvenIfCancelledCallbackRuns() {
    let owner = session()
    let target = FakeQuickPasteTarget(2)
    let id = owner.begin(frontmost: target, popupActive: false)!
    paste(owner, id)
    let oldCallback = continuation
    let next = owner.begin(frontmost: FakeQuickPasteTarget(3), popupActive: false)!
    oldCallback?()
    owner.invalidate()
    oldCallback?()
    XCTAssertGreaterThan(next, id)
    XCTAssertEqual(cancellations, 1)
    XCTAssertEqual(results, [false])
    XCTAssertEqual(submissions, 0)
  }

  func testOwnerReleaseCancelsPendingResultBeforeLateCallback() {
    var owner: QuickPastePasteSession? = session()
    let id = owner!.begin(frontmost: FakeQuickPasteTarget(2), popupActive: false)!
    paste(owner!, id)
    let callback = continuation
    owner = nil
    callback?()
    XCTAssertEqual(results, [false])
    XCTAssertEqual(cancellations, 1)
    XCTAssertEqual(submissions, 0)
  }

  func testReentrantInvalidationDuringActivationStopsBeforeScheduling() {
    let owner = session()
    let target = FakeQuickPasteTarget(2)
    foreground = 3
    target.onActivate = { owner.invalidate() }
    let id = owner.begin(frontmost: target, popupActive: false)!
    paste(owner, id)
    XCTAssertEqual(results, [false])
    XCTAssertNil(continuation)
    XCTAssertEqual(submissions, 0)
  }

  func testCompleteEventConstructionFailurePostsNoHalfChord() {
    for failure in [0, 1, 2] {
      var posts = 0
      var creations = 0
      let submit = MacosAccessibility.preparePaste(
        source: { failure == 0 ? nil : CGEventSource(stateID: .combinedSessionState) },
        event: { source, down in
          creations += 1
          if creations == failure { return nil }
          return CGEvent(keyboardEventSource: source, virtualKey: 9, keyDown: down)
        },
        post: { _ in posts += 1 }
      )
      XCTAssertNil(submit)
      submit?()
      XCTAssertEqual(posts, 0)
    }
    var posts = 0
    let submit = MacosAccessibility.preparePaste(post: { _ in posts += 1 })
    XCTAssertNotNil(submit)
    submit?()
    XCTAssertEqual(posts, 2)
  }
}

final class QuickPasteOpeningTests: XCTestCase {
  private func session() -> QuickPastePasteSession {
    QuickPastePasteSession(ownPID: 1, foreground: { 2 }, trusted: { true },
                          prepareInput: { nil }, schedule: { _ in {} })
  }

  func testSupersededShowSuccessAndFailureDoNotHideNewPresentation() {
    for oldShowSucceeds in [true, false] {
      let owner = session()
      let id = owner.begin(frontmost: FakeQuickPasteTarget(2), popupActive: false)!
      let replacement = FakeQuickPasteTarget(3)
      var newerID: Int64 = 0
      var hides = 0
      var opened = 0
      var positions = 0
      var newerOpened = 0
      let shown = presentQuickPaste(
        id: id, session: owner,
        position: { positions += 1; return true },
        show: {
          newerID = owner.begin(frontmost: replacement, popupActive: false)!
          XCTAssertTrue(presentQuickPaste(
            id: newerID, session: owner, position: { true }, show: { true },
            hide: { hides += 1 }, opened: { newerOpened += 1 }
          ))
          return oldShowSucceeds
        },
        hide: { hides += 1 }, opened: { opened += 1 }
      )
      XCTAssertFalse(shown)
      XCTAssertEqual(owner.id, newerID)
      XCTAssertTrue(owner.target === replacement)
      XCTAssertEqual(positions, 1)
      XCTAssertEqual(newerOpened, 1)
      XCTAssertEqual(hides, 0)
      XCTAssertEqual(opened, 0)
    }
  }

  func testSupersededPositionStopsBeforeOldShow() {
    let owner = session()
    let id = owner.begin(frontmost: FakeQuickPasteTarget(2), popupActive: false)!
    let replacement = FakeQuickPasteTarget(3)
    var shows = 0
    var hides = 0
    var opened = 0
    XCTAssertFalse(presentQuickPaste(
      id: id, session: owner,
      position: { _ = owner.begin(frontmost: replacement, popupActive: false); return true },
      show: { shows += 1; return true }, hide: { hides += 1 }, opened: { opened += 1 }
    ))
    XCTAssertTrue(owner.target === replacement)
    XCTAssertEqual(shows, 0)
    XCTAssertEqual(hides, 0)
    XCTAssertEqual(opened, 0)
  }

  func testSameOwnerPartialShowFailureInvalidatesAndHidesOnce() {
    let owner = session()
    let id = owner.begin(frontmost: FakeQuickPasteTarget(2), popupActive: false)!
    var hides = 0
    var opened = 0
    XCTAssertFalse(presentQuickPaste(
      id: id, session: owner, position: { true }, show: { false },
      hide: { hides += 1 }, opened: { opened += 1 }
    ))
    XCTAssertEqual(owner.id, 0)
    XCTAssertNil(owner.target)
    XCTAssertEqual(hides, 1)
    XCTAssertEqual(opened, 0)
  }
}
