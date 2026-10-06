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

  func detach() { root.detach() }
}

private final class ProtectedLayerNode {
  private static let context = CIContext(options: [.cacheIntermediates: false])
  private let source: CALayer
  private var children: [ObjectIdentifier: ProtectedLayerNode] = [:]
  private var observations: [NSKeyValueObservation] = []
  private var display: AVSampleBufferDisplayLayer?
  private var originalOpacity: Float?
  private var reconciling = false
  private var detached = false
  private var bufferPool: CVPixelBufferPool?
  private var bufferSize = CGSize.zero
  private var failure = false

  var healthy: Bool { !failure && children.values.allSatisfy(\.healthy) }

  init(source: CALayer) {
    self.source = source
    observations.append(source.observe(\.sublayers) { [weak self] _, _ in self?.reconcile() })
    observations.append(source.observe(\.contents) { [weak self] _, _ in self?.present() })
    observations.append(source.observe(\.bounds) { [weak self] _, _ in self?.layout() })
    observations.append(source.observe(\.position) { [weak self] _, _ in self?.layout() })
    observations.append(source.observe(\.anchorPoint) { [weak self] _, _ in self?.layout() })
    observations.append(source.observe(\.transform) { [weak self] _, _ in self?.layout() })
    observations.append(source.observe(\.contentsScale) { [weak self] _, _ in self?.layout() })
    reconcile()
    present()
  }

  func detach() {
    guard !detached else { return }
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
      children[ObjectIdentifier(layer)] = ProtectedLayerNode(source: layer)
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
    guard !detached, let raw = source.contents,
          CFGetTypeID(raw as CFTypeRef) == IOSurfaceGetTypeID() else { return }
    let surface = unsafeBitCast(raw as AnyObject, to: IOSurfaceRef.self)
    if display == nil {
      let display = AVSampleBufferDisplayLayer()
      display.preventsCapture = true
      display.videoGravity = .resize
      self.display = display
      originalOpacity = source.opacity
      source.opacity = 0
      source.superlayer?.insertSublayer(display, above: source)
    }
    guard let display else { return }
    layout()
    var unmanaged: Unmanaged<CVPixelBuffer>?
    guard CVPixelBufferCreateWithIOSurface(kCFAllocatorDefault, surface, nil, &unmanaged) == kCVReturnSuccess,
          let buffer = unmanaged?.takeRetainedValue() else { failure = true; return }
    let size = CGSize(width: CVPixelBufferGetWidth(buffer), height: CVPixelBufferGetHeight(buffer))
    if bufferPool == nil || bufferSize != size {
      bufferSize = size
      let attributes: [CFString: Any] = [
        kCVPixelBufferWidthKey: Int(size.width),
        kCVPixelBufferHeightKey: Int(size.height),
        kCVPixelBufferPixelFormatTypeKey: kCVPixelFormatType_32BGRA,
        kCVPixelBufferIOSurfacePropertiesKey: [:],
        kCVPixelBufferMetalCompatibilityKey: true,
      ]
      guard CVPixelBufferPoolCreate(kCFAllocatorDefault, nil, attributes as CFDictionary,
        &bufferPool) == kCVReturnSuccess else { failure = true; return }
    }
    var rendered: CVPixelBuffer?
    guard let pool = bufferPool,
          CVPixelBufferPoolCreatePixelBuffer(kCFAllocatorDefault, pool, &rendered) == kCVReturnSuccess,
          let rendered else { failure = true; return }
    // Flutter uses a wide-gamut IOSurface format that the video renderer cannot
    // display directly. Core Image converts it on the GPU; off uses Flutter's
    // original compositor without this pass.
    Self.context.render(CIImage(cvPixelBuffer: buffer), to: rendered)
    var format: CMVideoFormatDescription?
    guard CMVideoFormatDescriptionCreateForImageBuffer(allocator: kCFAllocatorDefault,
      imageBuffer: rendered, formatDescriptionOut: &format) == noErr,
      let format else { failure = true; return }
    var sample: CMSampleBuffer?
    var timing = CMSampleTimingInfo(duration: .invalid, presentationTimeStamp: .zero,
                                  decodeTimeStamp: .invalid)
    guard CMSampleBufferCreateReadyWithImageBuffer(allocator: kCFAllocatorDefault,
      imageBuffer: rendered, formatDescription: format, sampleTiming: &timing,
      sampleBufferOut: &sample) == noErr, let sample else { failure = true; return }
    if let attachments = CMSampleBufferGetSampleAttachmentsArray(sample, createIfNecessary: true) {
      let dictionary = unsafeBitCast(CFArrayGetValueAtIndex(attachments, 0), to: CFMutableDictionary.self)
      CFDictionarySetValue(dictionary,
        Unmanaged.passUnretained(kCMSampleAttachmentKey_DisplayImmediately).toOpaque(),
        Unmanaged.passUnretained(kCFBooleanTrue).toOpaque())
    }
    if #available(macOS 15.0, *) { display.sampleBufferRenderer.enqueue(sample) }
    else { display.enqueue(sample) }
  }
}
