import AppKit
import AVFoundation
import IOSurface
import CoreImage

/// Presents Flutter's IOSurfaces through capture-protected sample buffers.
/// Original compositor layers retain their frame and surface ownership.
final class CaptureProtectedLayerTree {
  private let root: ProtectedLayerNode

  init(layer: CALayer) {
    root = ProtectedLayerNode(source: layer)
  }

  var healthy: Bool { root.healthy }

  deinit { root.detach() }

  func suspend() { root.suspend() }
  func resume() { root.resume() }
  func detach() { root.detach() }
}

/// Coalesces compositor changes without retaining a second source frame.
final class CaptureProtectedFrameDelivery {
  private let enqueue: (@escaping () -> Void) -> Void
  private let render: () -> Bool
  private var pending = false
  private var scheduled = false
  private var active = true
  private var generation = 0

  init(
    enqueue: @escaping (@escaping () -> Void) -> Void = { DispatchQueue.main.async(execute: $0) },
    render: @escaping () -> Bool
  ) {
    self.enqueue = enqueue
    self.render = render
  }

  func request() {
    guard active else { return }
    pending = true
    guard !scheduled else { return }
    scheduled = true
    let scheduledGeneration = generation
    enqueue { [weak self] in
      guard let self, self.active, self.generation == scheduledGeneration else { return }
      self.scheduled = false
      guard self.pending else { return }
      self.pending = false
      if !self.render() { self.pending = true }
    }
  }

  func suspend() {
    active = false
    pending = false
    scheduled = false
    generation += 1
  }

  func resume() {
    guard !active else { return }
    active = true
    request()
  }
}

/// Bounds converted frames even when AVFoundation retains buffers for display.
final class CaptureProtectedPixelBufferPool {
  static let maximumBufferCount = 3
  private let available: () -> Void
  private var pool: CVPixelBufferPool?
  private var size = CGSize.zero
  private var observer: NSObjectProtocol?

  init(available: @escaping () -> Void) { self.available = available }

  deinit { reset() }

  func buffer(width: Int, height: Int) -> (CVReturn, CVPixelBuffer?) {
    let requestedSize = CGSize(width: width, height: height)
    if pool == nil || size != requestedSize {
      reset()
      let attributes: [CFString: Any] = [
        kCVPixelBufferWidthKey: width,
        kCVPixelBufferHeightKey: height,
        kCVPixelBufferPixelFormatTypeKey: kCVPixelFormatType_32BGRA,
        kCVPixelBufferIOSurfacePropertiesKey: [:],
        kCVPixelBufferMetalCompatibilityKey: true,
      ]
      let status = CVPixelBufferPoolCreate(kCFAllocatorDefault, nil, attributes as CFDictionary, &pool)
      guard status == kCVReturnSuccess, let pool else { return (status, nil) }
      size = requestedSize
      observer = NotificationCenter.default.addObserver(
        forName: Notification.Name(kCVPixelBufferPoolFreeBufferNotification as String),
        object: pool,
        queue: .main
      ) { [weak self] _ in self?.available() }
    }
    guard let pool else { return (kCVReturnInvalidPoolAttributes, nil) }
    let limits = [kCVPixelBufferPoolAllocationThresholdKey: Self.maximumBufferCount] as CFDictionary
    var buffer: CVPixelBuffer?
    let status = CVPixelBufferPoolCreatePixelBufferWithAuxAttributes(
      kCFAllocatorDefault, pool, limits, &buffer
    )
    return (status, buffer)
  }

  func reset() {
    if let observer { NotificationCenter.default.removeObserver(observer) }
    observer = nil
    if let pool { CVPixelBufferPoolFlush(pool, .excessBuffers) }
    pool = nil
    size = .zero
  }
}

/// Preserves Flutter's color space when converting frames for the video renderer.
final class CaptureProtectedFrameConverter {
  private static let context = CIContext(options: [.cacheIntermediates: false])
  private static let outputColorSpace = CGColorSpace(name: CGColorSpace.sRGB)!

  static func render(_ source: CVPixelBuffer, to destination: CVPixelBuffer) {
    let image = CIImage(cvPixelBuffer: source, options: [.colorSpace: colorSpace(of: source)])
    context.render(image, to: destination, bounds: image.extent, colorSpace: outputColorSpace)
    // The video renderer needs the same transfer function as the encoded pixels.
    // Untagged RGB samples leave it to choose a video color space and gamma.
    CVBufferSetAttachment(destination, kCVImageBufferCGColorSpaceKey,
                          outputColorSpace, .shouldPropagate)
    CVBufferSetAttachment(destination, kCVImageBufferColorPrimariesKey,
                          kCVImageBufferColorPrimaries_ITU_R_709_2, .shouldPropagate)
    CVBufferSetAttachment(destination, kCVImageBufferTransferFunctionKey,
                          kCVImageBufferTransferFunction_sRGB, .shouldPropagate)
    if let surface = CVPixelBufferGetIOSurface(destination)?.takeUnretainedValue() {
      IOSurfaceSetValue(surface, kIOSurfaceColorSpace, CGColorSpace.sRGB)
    }
  }

  private static func colorSpace(of buffer: CVPixelBuffer) -> CGColorSpace {
    if let surface = CVPixelBufferGetIOSurface(buffer)?.takeUnretainedValue(),
       let profile = IOSurfaceCopyValue(surface, kIOSurfaceColorSpace),
       let colorSpace = CGColorSpace(propertyListPlist: profile) {
      return colorSpace
    }
    return CVImageBufferGetColorSpace(buffer)?.takeUnretainedValue() ?? CGColorSpace(name:
      CVPixelBufferGetPixelFormatType(buffer) == kCVPixelFormatType_40ARGBLEWideGamut
        ? CGColorSpace.extendedSRGB : CGColorSpace.sRGB)!
  }
}

private final class ProtectedLayerNode {
  private let source: CALayer
  private var children: [ObjectIdentifier: ProtectedLayerNode] = [:]
  private var observations: [NSKeyValueObservation] = []
  private var display: AVSampleBufferDisplayLayer?
  private var originalOpacity: Float?
  private var reconciling = false
  private var detached = false
  private var suspended: Bool
  private var waitingForRenderer = false
  private var waitingForBuffer = false
  private var renderedSize = CGSize.zero
  private var resizing = false
  private var generation = 0
  private var failure = false
  private lazy var delivery = CaptureProtectedFrameDelivery { [weak self] in
    self?.renderLatestFrame() ?? true
  }
  private lazy var buffers = CaptureProtectedPixelBufferPool { [weak self] in
    guard let self, self.waitingForBuffer else { return }
    self.waitingForBuffer = false
    self.delivery.request()
  }

  var healthy: Bool { !failure && children.values.allSatisfy(\.healthy) }

  init(source: CALayer, suspended: Bool = false) {
    self.source = source
    self.suspended = suspended
    observations.append(source.observe(\.sublayers) { [weak self] _, _ in self?.reconcile() })
    observations.append(source.observe(\.contents) { [weak self] _, _ in self?.present() })
    observations.append(source.observe(\.bounds) { [weak self] _, _ in self?.layout() })
    observations.append(source.observe(\.position) { [weak self] _, _ in self?.layout() })
    observations.append(source.observe(\.anchorPoint) { [weak self] _, _ in self?.layout() })
    observations.append(source.observe(\.transform) { [weak self] _, _ in self?.layout() })
    observations.append(source.observe(\.contentsScale) { [weak self] _, _ in self?.layout() })
    if suspended { delivery.suspend() }
    reconcile()
    present()
  }

  func suspend() {
    suspended = true
    delivery.suspend()
    for child in children.values { child.suspend() }
    clearFrame()
  }

  func resume() {
    guard !detached, suspended else { return }
    suspended = false
    for child in children.values { child.resume() }
    delivery.resume()
  }

  func detach() {
    guard !detached else { return }
    suspend()
    detached = true
    observations.removeAll()
    for child in children.values { child.detach() }
    children.removeAll()
    if let opacity = originalOpacity { source.opacity = opacity }
    display?.removeFromSuperlayer()
    display = nil
  }

  private func reconcile() {
    guard !detached, !reconciling else { return }
    reconciling = true
    defer { reconciling = false }
    let layers = (source.sublayers ?? []).filter { !($0 is AVSampleBufferDisplayLayer) }
    let present = Set(layers.map(ObjectIdentifier.init))
    for id in Array(children.keys) where !present.contains(id) {
      children.removeValue(forKey: id)?.detach()
    }
    for layer in layers where children[ObjectIdentifier(layer)] == nil {
      children[ObjectIdentifier(layer)] = ProtectedLayerNode(source: layer, suspended: suspended)
    }
  }

  private func layout() {
    guard let display else { return }
    display.bounds = source.bounds
    display.position = source.position
    display.anchorPoint = source.anchorPoint
    display.transform = source.transform
    display.contentsScale = source.contentsScale
    display.zPosition = source.zPosition
    display.isGeometryFlipped = source.isGeometryFlipped
  }

  private func present() {
    guard !detached else { return }
    guard let raw = source.contents,
          CFGetTypeID(raw as CFTypeRef) == IOSurfaceGetTypeID() else {
      clearFrame()
      return
    }
    if display == nil {
      let display = AVSampleBufferDisplayLayer()
      display.preventsCapture = true
      display.videoGravity = .resize
      self.display = display
      originalOpacity = source.opacity
      source.opacity = 0
      source.superlayer?.insertSublayer(display, above: source)
    }
    // Hide the original synchronously, before an asynchronously prepared frame
    // or a newly opened window could expose the unprotected compositor.
    layout()
    delivery.request()
  }

  private func clearFrame() {
    generation += 1
    resizing = false
    renderedSize = .zero
    stopWaitingForRenderer()
    waitingForBuffer = false
    if let display {
      if #available(macOS 15.0, *) {
        display.sampleBufferRenderer.flush(removingDisplayedImage: true, completionHandler: nil)
      } else {
        display.flushAndRemoveImage()
      }
    }
    buffers.reset()
  }

  private func stopWaitingForRenderer() {
    guard waitingForRenderer, let display else { return }
    waitingForRenderer = false
    if #available(macOS 15.0, *) { display.sampleBufferRenderer.stopRequestingMediaData() }
    else { display.stopRequestingMediaData() }
  }

  private func waitForRenderer(_ display: AVSampleBufferDisplayLayer) {
    guard !waitingForRenderer else { return }
    waitingForRenderer = true
    let ready: @Sendable () -> Void = { [weak self] in
      guard let self, !self.detached, !self.suspended else { return }
      self.stopWaitingForRenderer()
      self.delivery.request()
    }
    if #available(macOS 15.0, *) {
      display.sampleBufferRenderer.requestMediaDataWhenReady(on: .main, using: ready)
    } else {
      display.requestMediaDataWhenReady(on: .main, using: ready)
    }
  }

  private func renderLatestFrame() -> Bool {
    guard !detached, !suspended, let display, let raw = source.contents,
          CFGetTypeID(raw as CFTypeRef) == IOSurfaceGetTypeID() else { return true }
    guard !resizing else { return false }
    let ready: Bool
    if #available(macOS 15.0, *) { ready = display.sampleBufferRenderer.isReadyForMoreMediaData }
    else { ready = display.isReadyForMoreMediaData }
    guard ready else { waitForRenderer(display); return false }
    stopWaitingForRenderer()
    let surface = unsafeBitCast(raw as AnyObject, to: IOSurfaceRef.self)
    var unmanaged: Unmanaged<CVPixelBuffer>?
    guard CVPixelBufferCreateWithIOSurface(kCFAllocatorDefault, surface, nil, &unmanaged) == kCVReturnSuccess,
          let buffer = unmanaged?.takeRetainedValue() else { failure = true; return true }
    let size = CGSize(width: CVPixelBufferGetWidth(buffer), height: CVPixelBufferGetHeight(buffer))
    if renderedSize != .zero && renderedSize != size {
      // Drain queued frames of the old size before creating another pool.
      // Keep the protected displayed frame until its replacement is ready.
      if #available(macOS 15.0, *) {
        resizing = true
        let resizingGeneration = generation
        display.sampleBufferRenderer.flush(removingDisplayedImage: false) { [weak self] in
          DispatchQueue.main.async { [weak self] in
            guard let self, self.generation == resizingGeneration else { return }
            self.buffers.reset()
            self.renderedSize = .zero
            self.resizing = false
            self.delivery.request()
          }
        }
        return false
      }
      display.flush()
      buffers.reset()
    }
    let (status, rendered) = buffers.buffer(
      width: CVPixelBufferGetWidth(buffer), height: CVPixelBufferGetHeight(buffer)
    )
    if status == kCVReturnWouldExceedAllocationThreshold {
      waitingForBuffer = true
      return false
    }
    guard status == kCVReturnSuccess, let rendered else { failure = true; return true }
    waitingForBuffer = false
    renderedSize = size
    // Flutter uses a wide-gamut IOSurface format that the video renderer cannot
    // display directly. Core Image converts it on the GPU; off uses Flutter's
    // original compositor without this pass.
    CaptureProtectedFrameConverter.render(buffer, to: rendered)
    var format: CMVideoFormatDescription?
    guard CMVideoFormatDescriptionCreateForImageBuffer(allocator: kCFAllocatorDefault,
      imageBuffer: rendered, formatDescriptionOut: &format) == noErr,
      let format else { failure = true; return true }
    var sample: CMSampleBuffer?
    var timing = CMSampleTimingInfo(duration: .invalid, presentationTimeStamp: .zero,
                                  decodeTimeStamp: .invalid)
    guard CMSampleBufferCreateReadyWithImageBuffer(allocator: kCFAllocatorDefault,
      imageBuffer: rendered, formatDescription: format, sampleTiming: &timing,
      sampleBufferOut: &sample) == noErr, let sample else { failure = true; return true }
    if let attachments = CMSampleBufferGetSampleAttachmentsArray(sample, createIfNecessary: true) {
      let dictionary = unsafeBitCast(CFArrayGetValueAtIndex(attachments, 0), to: CFMutableDictionary.self)
      CFDictionarySetValue(dictionary,
        Unmanaged.passUnretained(kCMSampleAttachmentKey_DisplayImmediately).toOpaque(),
        Unmanaged.passUnretained(kCFBooleanTrue).toOpaque())
    }
    if #available(macOS 15.0, *) { display.sampleBufferRenderer.enqueue(sample) }
    else { display.enqueue(sample) }
    return true
  }
}
