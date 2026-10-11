import NormSDK
import SwiftUI

struct ContentView: View {
    @EnvironmentObject private var model: ConsoleModel
    @State private var tab = 0

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                ConnectionBar()
                Picker("View", selection: $tab) {
                    Text("Log").tag(0)
                    Text("Commands").tag(1)
                }
                .pickerStyle(.segmented)
                .padding(.horizontal)
                .padding(.bottom, 8)
                if tab == 0 { LogView() } else { CommandsView() }
            }
            .navigationTitle("NormConsole")
            .navigationBarTitleDisplayMode(.inline)
        }
    }
}

private struct ConnectionBar: View {
    @EnvironmentObject private var model: ConsoleModel

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Picker("Target", selection: $model.target) {
                ForEach(ConsoleModel.Target.allCases) { Text($0.rawValue).tag($0) }
            }
            .pickerStyle(.segmented)
            HStack {
                if model.target == .bridge {
                    TextField("host:port", text: $model.bridgeAddress)
                        .textFieldStyle(.roundedBorder)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .accessibilityIdentifier("bridgeAddress")
                }
                Button(model.connected ? "Reconnect" : "Connect") {
                    Task { await model.connect() }
                }
                .buttonStyle(.borderedProminent)
                .accessibilityIdentifier("connect")
            }
            Text(model.status)
                .font(.caption)
                .foregroundStyle(.secondary)
                .accessibilityIdentifier("status")
        }
        .padding()
    }
}

private struct LogView: View {
    @EnvironmentObject private var model: ConsoleModel
    @State private var raw = ""

    var body: some View {
        VStack(spacing: 0) {
            ScrollViewReader { proxy in
                List(model.log) { line in
                    VStack(alignment: .leading, spacing: 2) {
                        Text(prefix(line.kind) + line.text)
                            .font(.system(.footnote, design: .monospaced))
                            .foregroundStyle(color(line.kind))
                        if let detail = line.detail {
                            Text(detail).font(.caption2).foregroundStyle(.secondary)
                        }
                    }
                    .id(line.id)
                    .accessibilityElement(children: .combine)
                    .accessibilityIdentifier("log")
                }
                .listStyle(.plain)
                .onChange(of: model.log.count) { _ in
                    if let last = model.log.last { proxy.scrollTo(last.id, anchor: .bottom) }
                }
            }
            HStack {
                TextField("6F 08 70 01 00 01 8F", text: $raw)
                    .font(.system(.body, design: .monospaced))
                    .textFieldStyle(.roundedBorder)
                    .textInputAutocapitalization(.characters)
                    .autocorrectionDisabled()
                Button("Send") { model.sendRaw(raw, remote: false) }
                    .disabled(!model.connected)
                Button("Clear") { model.clear() }
            }
            .padding()
        }
    }

    private func prefix(_ kind: LogLine.Kind) -> String {
        switch kind {
        case .phone: return "→ "
        case .watch: return "← "
        case .result: return "= "
        case .info: return "· "
        }
    }

    private func color(_ kind: LogLine.Kind) -> Color {
        switch kind {
        case .phone: return .blue
        case .watch: return .primary
        case .result: return .green
        case .info: return .secondary
        }
    }
}

private struct CommandsView: View {
    @EnvironmentObject private var model: ConsoleModel
    @State private var search = ""

    var body: some View {
        List(filtered, id: \.id) { spec in
            HStack {
                NavigationLink {
                    CommandDetail(spec: spec)
                } label: {
                    VStack(alignment: .leading) {
                        Text(spec.title)
                        Text("\(spec.id) · \(support(spec.firmware))")
                            .font(.caption).foregroundStyle(.secondary)
                    }
                }
                Button("Send") { model.send(spec) }
                    .buttonStyle(.bordered)
                    .disabled(!model.connected || isDestructive(kind: spec.kind))
                    .accessibilityIdentifier("send.\(spec.id)")
            }
        }
        .searchable(text: $search)
    }

    private var filtered: [CommandSpec] {
        search.isEmpty ? model.catalog : model.catalog.filter {
            ($0.id + $0.title).localizedCaseInsensitiveContains(search)
        }
    }
}

private struct CommandDetail: View {
    let spec: CommandSpec

    var body: some View {
        List {
            Section("Summary") { Text(spec.summary.isEmpty ? spec.title : spec.summary) }
            Section("Wire") {
                LabeledContent("Frame", value: String(format: "%@ 0x%02X", spec.format == .frame6f ? "6F" : "6E", spec.command))
                LabeledContent("Reply", value: spec.responseCommand.map { String(format: "0x%02X", $0) } ?? "none")
                Text(spec.requestLayout)
                Text(spec.responseLayout)
            }
            Section("Example") {
                Text(encodeRequest(request: exampleRequest(kind: spec.kind)).map { hexString(data: $0.data) }.joined(separator: "\n"))
                    .font(.system(.footnote, design: .monospaced))
            }
            Section("Firmware") { Text(support(spec.firmware)) }
            if !spec.notes.isEmpty { Section("Notes") { Text(spec.notes) } }
        }
        .navigationTitle(spec.id)
    }
}

private func support(_ firmware: FirmwareSupport) -> String {
    switch firmware {
    case .stock: return "stock"
    case .noStockHandler: return "no stock handler"
    case .ignored6e: return "6E, ignored by stock"
    case .patched(let feature): return "patched: \(feature)"
    }
}
