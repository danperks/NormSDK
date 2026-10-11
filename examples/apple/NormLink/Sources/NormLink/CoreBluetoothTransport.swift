import CoreBluetooth
import Foundation
import NormSDK

/// Connects to the first advertising Norm Two (or one whose name starts with
/// `namePrefix`) and wires its command characteristics.
public final class CoreBluetoothTransport: NSObject, WatchTransport, @unchecked Sendable {
    public var onNotify: ((Channel, Data) -> Void)?
    public var onStatus: ((String) -> Void)?

    private let namePrefix: String?
    private let queue = DispatchQueue(label: "normlink.ble")
    private let profile = gattProfile()
    private var central: CBCentralManager!
    private var peripheral: CBPeripheral?
    private var writeChars: [WriteChannel: CBCharacteristic] = [:]
    private var connectContinuation: CheckedContinuation<Void, Error>?

    public init(namePrefix: String? = nil) {
        self.namePrefix = namePrefix
        super.init()
    }

    public func connect() async throws {
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            queue.async {
                self.connectContinuation = continuation
                self.central = CBCentralManager(delegate: self, queue: self.queue)
                self.queue.asyncAfter(deadline: .now() + 20) { self.finish(TransportError.notFound) }
            }
        }
    }

    public func write(_ data: Data, channel: WriteChannel) {
        queue.async {
            guard let peripheral = self.peripheral, let char = self.writeChars[channel] else { return }
            let type: CBCharacteristicWriteType = char.properties.contains(.write) ? .withResponse : .withoutResponse
            let size = max(20, peripheral.maximumWriteValueLength(for: type))
            for chunk in chunkFrame(frame: data, maxLen: UInt32(size)) {
                peripheral.writeValue(chunk, for: char, type: type)
            }
        }
    }

    public func disconnect() {
        queue.async {
            if let peripheral = self.peripheral { self.central?.cancelPeripheralConnection(peripheral) }
        }
    }

    private func finish(_ error: Error?) {
        guard let continuation = connectContinuation else { return }
        connectContinuation = nil
        if let error { continuation.resume(throwing: error) } else { continuation.resume() }
    }

    private func uuid(_ string: String) -> CBUUID { CBUUID(string: string) }
}

extension CoreBluetoothTransport: CBCentralManagerDelegate, CBPeripheralDelegate {
    public func centralManagerDidUpdateState(_ central: CBCentralManager) {
        switch central.state {
        case .poweredOn:
            onStatus?("Scanning for a Norm Two…")
            central.scanForPeripherals(withServices: nil)
        case .unauthorized:
            finish(TransportError.failed("Bluetooth permission denied"))
        case .poweredOff, .unsupported:
            finish(TransportError.failed("Bluetooth unavailable"))
        default:
            break
        }
    }

    public func centralManager(_ central: CBCentralManager, didDiscover peripheral: CBPeripheral,
                               advertisementData: [String: Any], rssi RSSI: NSNumber) {
        let name = peripheral.name ?? advertisementData[CBAdvertisementDataLocalNameKey] as? String
        guard isNormDeviceName(name: name), namePrefix.map({ name?.hasPrefix($0) == true }) ?? true else { return }
        central.stopScan()
        self.peripheral = peripheral
        peripheral.delegate = self
        onStatus?("Connecting to \(name ?? "watch")…")
        central.connect(peripheral)
    }

    public func centralManager(_ central: CBCentralManager, didConnect peripheral: CBPeripheral) {
        peripheral.discoverServices([uuid(profile.primaryService), uuid(profile.extendService)])
    }

    public func centralManager(_ central: CBCentralManager, didFailToConnect peripheral: CBPeripheral, error: Error?) {
        finish(error ?? TransportError.failed("Connection failed"))
    }

    public func centralManager(_ central: CBCentralManager, didDisconnectPeripheral peripheral: CBPeripheral, error: Error?) {
        onStatus?("Disconnected")
    }

    public func peripheral(_ peripheral: CBPeripheral, didDiscoverServices error: Error?) {
        for service in peripheral.services ?? [] {
            peripheral.discoverCharacteristics(nil, for: service)
        }
    }

    public func peripheral(_ peripheral: CBPeripheral, didDiscoverCharacteristicsFor service: CBService, error: Error?) {
        let notify = [profile.commandNotify, profile.remoteNotify, profile.transparentNotify].map(uuid)
        for char in service.characteristics ?? [] {
            if char.uuid == uuid(profile.commandWrite) { writeChars[.primary] = char }
            if char.uuid == uuid(profile.remoteWrite) { writeChars[.remote] = char }
            if notify.contains(char.uuid) { peripheral.setNotifyValue(true, for: char) }
        }
    }

    public func peripheral(_ peripheral: CBPeripheral, didUpdateNotificationStateFor characteristic: CBCharacteristic,
                           error: Error?) {
        guard characteristic.uuid == uuid(profile.commandNotify) else { return }
        if let error {
            finish(error)
        } else {
            onStatus?("Connected to \(peripheral.name ?? "watch")")
            finish(nil)
        }
    }

    public func peripheral(_ peripheral: CBPeripheral, didUpdateValueFor characteristic: CBCharacteristic, error: Error?) {
        guard let data = characteristic.value else { return }
        switch characteristic.uuid {
        case uuid(profile.commandNotify): onNotify?(.primary, data)
        case uuid(profile.remoteNotify): onNotify?(.remote, data)
        case uuid(profile.transparentNotify): onNotify?(.transparent, data)
        default: break
        }
    }
}
