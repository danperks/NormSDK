import Foundation
import Network
import NormSDK

/// Talks to `tools/emulator/bridge.py`: one JSON object per line over TCP.
public final class BridgeTransport: WatchTransport, @unchecked Sendable {
    public var onNotify: ((Channel, Data) -> Void)?
    public var onStatus: ((String) -> Void)?

    private let host: String
    private let port: UInt16
    private let queue = DispatchQueue(label: "normlink.bridge")
    private var connection: NWConnection?
    private var buffer = Data()

    public init(host: String = "127.0.0.1", port: UInt16 = 7333) {
        self.host = host
        self.port = port
    }

    public func connect() async throws {
        guard let port = NWEndpoint.Port(rawValue: port) else { throw TransportError.failed("Bad port") }
        let connection = NWConnection(host: NWEndpoint.Host(host), port: port, using: .tcp)
        self.connection = connection
        onStatus?("Connecting to \(host):\(port)…")
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            let once = Once()
            connection.stateUpdateHandler = { state in
                guard !once.done else { return }
                switch state {
                case .ready:
                    once.done = true
                    continuation.resume()
                case .failed(let error), .waiting(let error):
                    once.done = true
                    connection.cancel()
                    continuation.resume(throwing: TransportError.failed("Bridge: \(error.localizedDescription)"))
                default:
                    break
                }
            }
            connection.start(queue: queue)
        }
        receive()
    }

    public func write(_ data: Data, channel: WriteChannel) {
        let message: [String: String] = [
            "type": "write",
            "channel": channel == .remote ? "remote" : "primary",
            "data": data.map { String(format: "%02X", $0) }.joined(),
        ]
        guard var line = try? JSONSerialization.data(withJSONObject: message) else { return }
        line.append(0x0A)
        connection?.send(content: line, completion: .contentProcessed { _ in })
    }

    public func disconnect() {
        connection?.cancel()
        connection = nil
    }

    private func receive() {
        connection?.receive(minimumIncompleteLength: 1, maximumLength: 65_536) { [weak self] data, _, done, error in
            guard let self else { return }
            if let data { self.consume(data) }
            if done || error != nil {
                self.onStatus?("Bridge closed")
                return
            }
            self.receive()
        }
    }

    private func consume(_ data: Data) {
        buffer.append(data)
        while let newline = buffer.firstIndex(of: 0x0A) {
            let line = buffer[buffer.startIndex..<newline]
            buffer.removeSubrange(buffer.startIndex...newline)
            guard let object = try? JSONSerialization.jsonObject(with: line) as? [String: Any],
                  let type = object["type"] as? String else { continue }
            if type == "hello" {
                onStatus?("Connected to \(object["name"] as? String ?? "watch")")
            } else if type == "notify", let hex = object["data"] as? String, let bytes = Data(hex: hex) {
                onNotify?((object["channel"] as? String) == "remote" ? .remote : .primary, bytes)
            }
        }
    }
}

/// Touched only on the connection's queue.
private final class Once: @unchecked Sendable {
    var done = false
}

extension Data {
    init?(hex: String) {
        var bytes = [UInt8]()
        var index = hex.startIndex
        while index < hex.endIndex {
            guard let next = hex.index(index, offsetBy: 2, limitedBy: hex.endIndex),
                  let byte = UInt8(hex[index..<next], radix: 16) else { return nil }
            bytes.append(byte)
            index = next
        }
        self.init(bytes)
    }
}
