import Foundation
import NormSDK

/// A link to a watch: real Bluetooth or the emulator bridge.
public protocol WatchTransport: AnyObject {
    /// Called with each notification, on any queue.
    var onNotify: ((Channel, Data) -> Void)? { get set }
    /// Human-readable connection progress.
    var onStatus: ((String) -> Void)? { get set }
    func connect() async throws
    /// Writes one whole frame; the transport splits it to the link's MTU.
    func write(_ data: Data, channel: WriteChannel)
    func disconnect()
}

public enum TransportError: LocalizedError {
    case notFound
    case failed(String)

    public var errorDescription: String? {
        switch self {
        case .notFound: return "No Norm Two found."
        case .failed(let reason): return reason
        }
    }
}
