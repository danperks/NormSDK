import Foundation
import NormLink
import NormSDK

struct LogLine: Identifiable {
    enum Kind { case phone, watch, result, info }

    let id = UUID()
    let kind: Kind
    let text: String
    let detail: String?
}

@MainActor
final class ConsoleModel: ObservableObject {
    enum Target: String, CaseIterable, Identifiable {
        case bridge = "Emulator bridge"
        case bluetooth = "Bluetooth"
        var id: String { rawValue }
    }

    @Published var target: Target = .bridge
    @Published var bridgeAddress = "127.0.0.1:7333"
    @Published var status = "Not connected"
    @Published var connected = false
    @Published var log: [LogLine] = []

    let catalog = commandCatalog()
    private var link: NormLink?

    func autoConnectIfRequested() async {
        let defaults = UserDefaults.standard
        if let address = defaults.string(forKey: "bridge") {
            bridgeAddress = address
            target = .bridge
            await connect()
        }
    }

    func connect() async {
        link?.stop()
        let transport: WatchTransport
        switch target {
        case .bridge:
            let parts = bridgeAddress.split(separator: ":")
            transport = BridgeTransport(host: String(parts.first ?? "127.0.0.1"),
                                        port: UInt16(parts.last ?? "7333") ?? 7333)
        case .bluetooth:
            transport = CoreBluetoothTransport()
        }
        let link = NormLink(transport: transport)
        transport.onStatus = { [weak self] text in Task { @MainActor in self?.status = text } }
        link.onWrite = { [weak self] write in
            let line = LogLine(kind: .phone, text: hexString(data: write.data), detail: write.label)
            Task { @MainActor in self?.append(line) }
        }
        link.onEvent = { [weak self] event in
            guard let line = Self.line(for: event) else { return }
            Task { @MainActor in self?.append(line) }
        }
        self.link = link
        do {
            try await link.start()
            connected = true
        } catch {
            status = error.localizedDescription
            connected = false
        }
    }

    func send(_ spec: CommandSpec) {
        guard let link else { return }
        Task { await link.send(exampleRequest(kind: spec.kind)) }
    }

    func sendRaw(_ hex: String, remote: Bool) {
        let digits = hex.filter { !$0.isWhitespace }
        var bytes = [UInt8]()
        var index = digits.startIndex
        while index < digits.endIndex, let next = digits.index(index, offsetBy: 2, limitedBy: digits.endIndex),
              let byte = UInt8(digits[index..<next], radix: 16) {
            bytes.append(byte)
            index = next
        }
        guard let link, !bytes.isEmpty else { return }
        Task {
            await link.send(.raw(label: "raw", frame: Data(bytes), channel: remote ? .remote : .primary,
                                 responseCommand: nil, timeoutMs: 0))
        }
    }

    func clear() { log.removeAll() }

    private func append(_ line: LogLine) {
        log.append(line)
        if log.count > 500 { log.removeFirst(log.count - 500) }
    }

    nonisolated private static func line(for event: SessionEvent) -> LogLine? {
        switch event {
        case .received(let packet, let message, _):
            return LogLine(kind: .watch, text: hexString(data: packet.frame), detail: describeMessage(message: message))
        case .completed(let id, let response):
            return LogLine(kind: .result, text: "#\(id) \(describeResponse(response: response))", detail: nil)
        case .timedOut(let id, let partial):
            return LogLine(kind: .result, text: "#\(id) timed out", detail: partial.map { describeResponse(response: $0) })
        case .progress(let id, let received, let total, _):
            return LogLine(kind: .info, text: "#\(id) \(received)/\(total)", detail: nil)
        case .cancelled:
            return nil
        }
    }
}
