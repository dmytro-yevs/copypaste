import Darwin
import FlutterMacOS

/// Forwards OS memory pressure to every live engine and returns allocator
/// pages that the application process has already freed.
final class MacosMemoryPressure {
  static let shared = MacosMemoryPressure()
  private static let reliefChannel = "com.copypaste.app/memory"

  private final class Client {
    weak var engine: FlutterEngine?
    let channel: FlutterMethodChannel
    init(engine: FlutterEngine, channel: FlutterMethodChannel) {
      self.engine = engine
      self.channel = channel
    }
  }

  private var clients: [Client] = []
  private var source: DispatchSourceMemoryPressure?
  private var reliefScheduled = false

  func register(engine: FlutterEngine) {
    unregister(engine: engine)
    let channel = FlutterMethodChannel(
      name: Self.reliefChannel, binaryMessenger: engine.binaryMessenger
    )
    channel.setMethodCallHandler { [weak self] call, result in
      guard call.method == "releaseFreedMemory" else {
        result(FlutterMethodNotImplemented)
        return
      }
      self?.scheduleRelief()
      result(nil)
    }
    clients.append(Client(engine: engine, channel: channel))
    startMonitoring()
  }

  /// Called before an engine shuts down so pressure is never sent to it.
  func unregister(engine: FlutterEngine) {
    clients.removeAll { client in
      guard client.engine == nil || client.engine === engine else { return false }
      client.channel.setMethodCallHandler(nil)
      return true
    }
  }

  private func startMonitoring() {
    guard source == nil else { return }
    let source = DispatchSource.makeMemoryPressureSource(
      eventMask: [.warning, .critical], queue: .main
    )
    source.setEventHandler { [weak self] in self?.forwardPressure() }
    source.activate()
    self.source = source
  }

  private func forwardPressure() {
    let message = FlutterJSONMessageCodec.sharedInstance().encode(["type": "memoryPressure"])
    for client in clients {
      client.engine?.binaryMessenger.send(onChannel: "flutter/system", message: message)
    }
  }

  // Returns dirty free pages kept by the tiny and small allocators. It does not
  // drain the large-block cache, which Info.plist disables before launch.
  private func scheduleRelief() {
    guard !reliefScheduled else { return }
    reliefScheduled = true
    DispatchQueue.global(qos: .utility).async { [weak self] in
      _ = malloc_zone_pressure_relief(nil, 0)
      DispatchQueue.main.async { self?.reliefScheduled = false }
    }
  }
}
