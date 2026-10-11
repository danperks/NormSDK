import Foundation
import NormSDK

/// Drives a NormSDK `Session` over a transport: writes what the session asks
/// for, feeds notifications back, and runs its timers.
public final class NormLink: @unchecked Sendable {
    public let session = Session()
    /// Every session event, on the link's queue.
    public var onEvent: ((SessionEvent) -> Void)?
    /// Every write, before it goes out.
    public var onWrite: ((OutgoingWrite) -> Void)?

    public let transport: WatchTransport
    private let queue = DispatchQueue(label: "normlink.session")
    private var timer: DispatchSourceTimer?
    private var waiters: [UInt64: CheckedContinuation<SessionEvent?, Never>] = [:]

    public init(transport: WatchTransport) {
        self.transport = transport
        transport.onNotify = { [weak self] channel, data in
            self?.queue.async { self?.feed(channel, data) }
        }
    }

    public static var nowMs: UInt64 { UInt64(Date().timeIntervalSince1970 * 1000) }

    public func start() async throws {
        try await transport.connect()
        queue.sync {
            session.setLinkReady(ready: true)
            let timer = DispatchSource.makeTimerSource(queue: queue)
            timer.schedule(deadline: .now(), repeating: .milliseconds(20))
            timer.setEventHandler { [weak self] in self?.pump() }
            timer.resume()
            self.timer = timer
        }
    }

    public func stop() {
        queue.sync {
            timer?.cancel()
            timer = nil
            for event in session.clear() { deliver([event]) }
        }
        transport.disconnect()
    }

    /// Sends a request and waits for its completion or timeout. Returns nil
    /// for write-only requests once they are written.
    @discardableResult
    public func send(_ request: Request, priority: Priority = .normal) async -> SessionEvent? {
        await withCheckedContinuation { continuation in
            queue.async {
                let id = self.session.enqueue(request: request, priority: priority)
                self.waiters[id] = continuation
                self.pump()
            }
        }
    }

    private func pump() {
        for write in session.pollTransmit(nowMs: Self.nowMs) {
            onWrite?(write)
            transport.write(write.data, channel: write.channel)
            if !write.awaitsResponse { resolve(write.requestId, with: nil) }
        }
        deliver(session.tick(nowMs: Self.nowMs))
    }

    private func feed(_ channel: Channel, _ data: Data) {
        deliver(session.handleNotification(channel: channel, data: data, nowMs: Self.nowMs))
        pump()
    }

    private func deliver(_ events: [SessionEvent]) {
        for event in events {
            onEvent?(event)
            switch event {
            case .completed(let id, _), .timedOut(let id, _), .cancelled(let id):
                resolve(id, with: event)
            default:
                break
            }
        }
    }

    private func resolve(_ id: UInt64, with event: SessionEvent?) {
        waiters.removeValue(forKey: id)?.resume(returning: event)
    }
}

extension Packet {
    /// The frame as it appeared on the wire.
    public var frame: Data {
        format == .frame6f
            ? encodeFrame6f(command: command, action: action, payload: payload)
            : encodeFrame6e(command: command, payload: payload)
    }
}
