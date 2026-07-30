import Darwin
import Foundation

/// One blocking `AF_UNIX` stream connection, framed by newlines.
///
/// Why POSIX rather than `Network.framework`: `NWConnection` over
/// `NWEndpoint.unix(path:)` would work, but it drags an asynchronous state
/// machine and its own queue into a client whose entire job is one
/// request/response round trip. Sixty lines of `connect`/`read`/`write` with
/// explicit timeouts is the smaller surface *and* the one whose failure modes
/// map one-for-one onto `DaemonError`.
///
/// Every method here blocks. Nothing may call them from the main thread —
/// manifest 06 INV-37: a blocking IPC call on the UI thread freezes the menu
/// bar for the length of the socket timeout. `DaemonClient` owns the only
/// instances and confines them to its own dispatch queue.
///
/// This type never surfaces the socket path. Every failure is an errno the
/// caller maps to a pathless `DaemonError`.
final class UnixSocketChannel {
    /// Longer than any operation the daemon performs synchronously, short
    /// enough that a wedged service does not look like a hung app.
    static let defaultTimeout: TimeInterval = 5

    private var descriptor: Int32
    /// Bytes read past the end of the last frame. There is one frame per
    /// connection today, but a reader that discards a partial frame is a bug
    /// waiting for the day there are two.
    private var pending = Data()

    enum Failure: Error, Equatable {
        /// `sun_path` is 104 bytes on Darwin, and the path did not fit.
        case pathTooLong
        case cannotCreateSocket
        /// Includes "no such file" — the service is not listening.
        case cannotConnect
        case timedOut
        case closedByPeer
        case ioFailed
        /// A frame exceeded `maxFrameBytes` before a newline arrived.
        case frameTooLarge
    }

    init(path: String, timeout: TimeInterval = UnixSocketChannel.defaultTimeout) throws {
        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)

        let pathBytes = Array(path.utf8)
        let capacity = MemoryLayout.size(ofValue: address.sun_path)
        // Strictly less than: room for the terminating NUL.
        guard pathBytes.count < capacity else { throw Failure.pathTooLong }
        withUnsafeMutablePointer(to: &address.sun_path) { tuple in
            tuple.withMemoryRebound(to: CChar.self, capacity: capacity) { destination in
                for (offset, byte) in pathBytes.enumerated() {
                    destination[offset] = CChar(bitPattern: byte)
                }
                destination[pathBytes.count] = 0
            }
        }
        address.sun_len = UInt8(MemoryLayout<sockaddr_un>.size)

        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else { throw Failure.cannotCreateSocket }
        descriptor = fd

        // Without SO_NOSIGPIPE, writing to a socket the service has closed
        // raises SIGPIPE and kills the app. This is the macOS spelling; Linux
        // would use MSG_NOSIGNAL on send().
        var on: Int32 = 1
        setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &on, socklen_t(MemoryLayout<Int32>.size))
        setTimeout(timeout, for: SO_RCVTIMEO)
        setTimeout(timeout, for: SO_SNDTIMEO)

        let result = withUnsafePointer(to: &address) { pointer in
            pointer.withMemoryRebound(to: sockaddr.self, capacity: 1) { sockaddrPointer in
                Darwin.connect(fd, sockaddrPointer, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard result == 0 else {
            close()
            throw Failure.cannotConnect
        }
    }

    deinit { close() }

    func close() {
        if descriptor >= 0 {
            Darwin.close(descriptor)
            descriptor = -1
        }
    }

    private func setTimeout(_ seconds: TimeInterval, for option: Int32) {
        var value = timeval(
            tv_sec: Int(seconds),
            tv_usec: Int32((seconds - Double(Int(seconds))) * 1_000_000)
        )
        setsockopt(descriptor, SOL_SOCKET, option, &value, socklen_t(MemoryLayout<timeval>.size))
    }

    /// Write every byte, retrying short writes and `EINTR`.
    func write(_ data: Data) throws {
        try data.withUnsafeBytes { (raw: UnsafeRawBufferPointer) in
            guard let base = raw.baseAddress else { return }
            var written = 0
            while written < raw.count {
                let n = Darwin.write(descriptor, base + written, raw.count - written)
                if n > 0 {
                    written += n
                    continue
                }
                if n == 0 { throw Failure.closedByPeer }
                switch errno {
                case EINTR: continue
                case EAGAIN, EWOULDBLOCK: throw Failure.timedOut
                case EPIPE, ECONNRESET: throw Failure.closedByPeer
                default: throw Failure.ioFailed
                }
            }
        }
    }

    /// Read one newline-terminated frame and return it without the newline.
    ///
    /// The daemon closing the connection without answering is its read-timeout
    /// behaviour and is indistinguishable from it having gone away, so it
    /// surfaces as `closedByPeer` — which the client maps to "unreachable",
    /// exactly as the CLI does.
    func readFrame(limit: Int = maxFrameBytes) throws -> Data {
        var buffer = [UInt8](repeating: 0, count: 64 * 1024)

        while true {
            if let newline = pending.firstIndex(of: UInt8(ascii: "\n")) {
                let frame = pending[pending.startIndex..<newline]
                pending = pending[pending.index(after: newline)...]
                return Data(frame)
            }
            guard pending.count <= limit else { throw Failure.frameTooLarge }

            let n = buffer.withUnsafeMutableBytes { raw in
                Darwin.read(descriptor, raw.baseAddress, raw.count)
            }
            if n > 0 {
                pending.append(contentsOf: buffer[0..<n])
                continue
            }
            if n == 0 { throw Failure.closedByPeer }
            switch errno {
            case EINTR: continue
            case EAGAIN, EWOULDBLOCK: throw Failure.timedOut
            case ECONNRESET: throw Failure.closedByPeer
            default: throw Failure.ioFailed
            }
        }
    }
}
