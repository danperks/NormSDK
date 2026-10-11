import Foundation
import NormLink
import NormSDK

let usage = """
usage: normconsole (--bridge HOST:PORT | --ble [NAME]) [COMMAND ...]

Commands (as arguments, or one per line on stdin):
  list [text]        catalog entries
  info ID            one command's annotation
  send ID            send a command with its example parameters
  raw HEX [remote]   write a raw frame
  wait SECONDS       keep printing watch output
"""

func hex(_ data: Data) -> String { data.map { String(format: "%02X", $0) }.joined(separator: " ") }

let catalog = Dictionary(uniqueKeysWithValues: commandCatalog().map { ($0.id, $0) })

func run(_ line: String, on link: NormLink) async {
    let words = line.split(separator: " ").map(String.init)
    guard let verb = words.first else { return }
    let args = Array(words.dropFirst())
    switch verb {
    case "list":
        for spec in commandCatalog() where args.isEmpty || (spec.id + spec.title).localizedCaseInsensitiveContains(args[0]) {
            print("  \(spec.id.padding(toLength: 34, withPad: " ", startingAt: 0)) \(spec.title)")
        }
    case "info":
        guard let spec = args.first.flatMap({ catalog[$0] }) else { return print("unknown command") }
        print("  \(spec.title). \(spec.summary)\n  request:  \(spec.requestLayout)\n  response: \(spec.responseLayout)")
        print("  firmware: \(spec.firmware)  evidence: \(spec.evidence)")
        if !spec.notes.isEmpty { print("  \(spec.notes)") }
    case "send":
        guard let spec = args.first.flatMap({ catalog[$0] }) else { return print("unknown command (try list)") }
        if isDestructive(kind: spec.kind), link.transport is CoreBluetoothTransport {
            return print("refusing to send \(spec.id) to a real watch from the CLI")
        }
        await link.send(exampleRequest(kind: spec.kind))
    case "raw":
        let remote = args.last == "remote"
        guard let frame = Data(hexString: (remote ? args.dropLast() : args[...]).joined()) else { return print("bad hex") }
        await link.send(.raw(label: "cli", frame: frame, channel: remote ? .remote : .primary,
                             responseCommand: nil, timeoutMs: 0))
    case "wait":
        try? await Task.sleep(nanoseconds: UInt64((Double(args.first ?? "1") ?? 1) * 1e9))
    default:
        print("unknown command\n\(usage)")
    }
    try? await Task.sleep(nanoseconds: 200_000_000)
}

extension Data {
    init?(hexString: String) {
        var bytes = [UInt8]()
        var index = hexString.startIndex
        while index < hexString.endIndex {
            guard let next = hexString.index(index, offsetBy: 2, limitedBy: hexString.endIndex),
                  let byte = UInt8(hexString[index..<next], radix: 16) else { return nil }
            bytes.append(byte)
            index = next
        }
        self.init(bytes)
    }
}

var arguments = Array(CommandLine.arguments.dropFirst())
let transport: WatchTransport
switch arguments.first {
case "--bridge":
    arguments.removeFirst()
    let address = arguments.isEmpty ? "127.0.0.1:7333" : arguments.removeFirst()
    let parts = address.split(separator: ":")
    transport = BridgeTransport(host: String(parts.first ?? "127.0.0.1"), port: UInt16(parts.last ?? "7333") ?? 7333)
case "--ble":
    arguments.removeFirst()
    let name = arguments.first.flatMap { catalog[$0] == nil && !$0.contains(" ") ? $0 : nil }
    if name != nil { arguments.removeFirst() }
    transport = CoreBluetoothTransport(namePrefix: name)
default:
    print(usage)
    exit(2)
}

let link = NormLink(transport: transport)
transport.onStatus = { print("· \($0)") }
link.onWrite = { print("  phone  \(hex($0.data))  [\($0.label)]") }
link.onEvent = { event in
    switch event {
    case .received(let packet, let message, let consumedBy):
        print("  watch  \(hex(packet.frame))\(consumedBy.map { " (#\($0))" } ?? "")\n         \(describeMessage(message: message))")
    case .progress(let id, let received, let total, _):
        print("  #\(id) progress \(received)/\(total)")
    case .completed(let id, let response):
        print("  #\(id) → \(describeResponse(response: response))")
    case .timedOut(let id, let partial):
        print("  #\(id) timed out (\(partial.map { describeResponse(response: $0) } ?? "nothing"))")
    case .cancelled(let id):
        print("  #\(id) cancelled")
    }
}

let done = DispatchSemaphore(value: 0)
Task {
    do {
        try await link.start()
        if arguments.isEmpty {
            while let line = readLine() { await run(line, on: link) }
        } else {
            for command in arguments {
                print("> \(command)")
                await run(command, on: link)
            }
        }
    } catch {
        print("error: \(error.localizedDescription)")
    }
    done.signal()
}
done.wait()
