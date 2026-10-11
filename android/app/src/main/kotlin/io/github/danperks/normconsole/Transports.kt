package io.github.danperks.normconsole

import android.annotation.SuppressLint
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothGatt
import android.bluetooth.BluetoothGattCallback
import android.bluetooth.BluetoothGattCharacteristic
import android.bluetooth.BluetoothGattDescriptor
import android.bluetooth.BluetoothManager
import android.bluetooth.BluetoothProfile
import android.bluetooth.le.ScanCallback
import android.bluetooth.le.ScanResult
import android.content.Context
import io.github.danperks.normsdk.Channel
import io.github.danperks.normsdk.WriteChannel
import io.github.danperks.normsdk.chunkFrame
import io.github.danperks.normsdk.gattProfile
import io.github.danperks.normsdk.isNormDeviceName
import org.json.JSONObject
import java.io.BufferedReader
import java.io.InputStreamReader
import java.net.Socket
import java.util.ArrayDeque
import java.util.UUID
import kotlin.concurrent.thread

/** A link to a watch. Callbacks arrive on background threads. */
interface WatchTransport {
    var onNotify: (Channel, ByteArray) -> Unit
    var onStatus: (String) -> Unit
    fun connect()
    fun write(data: ByteArray, channel: WriteChannel)
    fun disconnect()
}

/** tools/emulator/bridge.py; the Android emulator reaches the host as 10.0.2.2. */
class BridgeTransport(private val host: String, private val port: Int) : WatchTransport {
    override var onNotify: (Channel, ByteArray) -> Unit = { _, _ -> }
    override var onStatus: (String) -> Unit = {}
    private var socket: Socket? = null

    override fun connect() {
        thread(name = "bridge") {
            try {
                onStatus("Connecting to $host:$port…")
                val socket = Socket(host, port).also { this.socket = it }
                val reader = BufferedReader(InputStreamReader(socket.getInputStream()))
                while (true) {
                    val message = JSONObject(reader.readLine() ?: break)
                    when (message.getString("type")) {
                        "hello" -> onStatus("Connected to ${message.optString("name")}")
                        "notify" -> {
                            val channel = if (message.optString("channel") == "remote") Channel.REMOTE else Channel.PRIMARY
                            onNotify(channel, message.getString("data").chunked(2).map { it.toInt(16).toByte() }.toByteArray())
                        }
                    }
                }
                onStatus("Bridge closed")
            } catch (e: Exception) {
                onStatus("Bridge: ${e.message}")
            }
        }
    }

    override fun write(data: ByteArray, channel: WriteChannel) {
        val line = JSONObject()
            .put("type", "write")
            .put("channel", if (channel == WriteChannel.REMOTE) "remote" else "primary")
            .put("data", data.joinToString("") { "%02X".format(it) })
        thread { runCatching { socket?.getOutputStream()?.write((line.toString() + "\n").toByteArray()) } }
    }

    override fun disconnect() {
        runCatching { socket?.close() }
    }
}

/** Scans for a Norm Two, connects, subscribes, and serialises GATT writes. */
@SuppressLint("MissingPermission")
class BleTransport(private val context: Context) : WatchTransport {
    override var onNotify: (Channel, ByteArray) -> Unit = { _, _ -> }
    override var onStatus: (String) -> Unit = {}

    private val profile = gattProfile()
    private val cccd = UUID.fromString("00002902-0000-1000-8000-00805f9b34fb")
    private val adapter = (context.getSystemService(Context.BLUETOOTH_SERVICE) as BluetoothManager).adapter
    private var gatt: BluetoothGatt? = null
    private var mtu = 23
    private val operations = ArrayDeque<(BluetoothGatt) -> Unit>()
    private var busy = false
    private val writes = mutableMapOf<WriteChannel, BluetoothGattCharacteristic>()

    private fun uuid(value: String): UUID =
        UUID.fromString(if (value.length == 4) "0000$value-0000-1000-8000-00805f9b34fb" else value)

    private val scan = object : ScanCallback() {
        override fun onScanResult(callbackType: Int, result: ScanResult) {
            val name = result.scanRecord?.deviceName ?: result.device.name
            if (!isNormDeviceName(name)) return
            adapter.bluetoothLeScanner.stopScan(this)
            onStatus("Connecting to $name…")
            gatt = result.device.connectGatt(context, false, callback, BluetoothDevice.TRANSPORT_LE)
        }
    }

    override fun connect() {
        onStatus("Scanning for a Norm Two…")
        adapter.bluetoothLeScanner.startScan(scan)
    }

    override fun write(data: ByteArray, channel: WriteChannel) {
        val characteristic = writes[channel] ?: return
        for (chunk in chunkFrame(data, (mtu - 3).toUInt())) {
            enqueue { g ->
                @Suppress("DEPRECATION")
                characteristic.value = chunk
                @Suppress("DEPRECATION")
                g.writeCharacteristic(characteristic)
            }
        }
    }

    override fun disconnect() {
        gatt?.close()
        gatt = null
    }

    @Synchronized
    private fun enqueue(operation: (BluetoothGatt) -> Unit) {
        operations.add(operation)
        if (!busy) next()
    }

    @Synchronized
    private fun next() {
        val g = gatt ?: return
        val operation = operations.poll()
        busy = operation != null
        operation?.invoke(g)
    }

    private val callback = object : BluetoothGattCallback() {
        override fun onConnectionStateChange(g: BluetoothGatt, status: Int, newState: Int) {
            if (newState == BluetoothProfile.STATE_CONNECTED) g.requestMtu(247) else onStatus("Disconnected")
        }

        override fun onMtuChanged(g: BluetoothGatt, mtu: Int, status: Int) {
            this@BleTransport.mtu = mtu
            g.discoverServices()
        }

        override fun onServicesDiscovered(g: BluetoothGatt, status: Int) {
            val notify = listOf(profile.commandNotify, profile.remoteNotify, profile.transparentNotify).map(::uuid)
            for (service in g.services) for (c in service.characteristics) {
                when (c.uuid) {
                    uuid(profile.commandWrite) -> writes[WriteChannel.PRIMARY] = c
                    uuid(profile.remoteWrite) -> writes[WriteChannel.REMOTE] = c
                    in notify -> enqueue { gatt ->
                        gatt.setCharacteristicNotification(c, true)
                        val descriptor = c.getDescriptor(cccd)
                        @Suppress("DEPRECATION")
                        descriptor.value = BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE
                        @Suppress("DEPRECATION")
                        gatt.writeDescriptor(descriptor)
                    }
                }
            }
            enqueue { onStatus("Connected to ${it.device.name}"); next() }
        }

        override fun onDescriptorWrite(g: BluetoothGatt, d: BluetoothGattDescriptor, status: Int) = next()

        override fun onCharacteristicWrite(g: BluetoothGatt, c: BluetoothGattCharacteristic, status: Int) = next()

        @Deprecated("Deprecated in API 33")
        override fun onCharacteristicChanged(g: BluetoothGatt, c: BluetoothGattCharacteristic) {
            @Suppress("DEPRECATION")
            val value = c.value ?: return
            when (c.uuid) {
                uuid(profile.commandNotify) -> onNotify(Channel.PRIMARY, value)
                uuid(profile.remoteNotify) -> onNotify(Channel.REMOTE, value)
                uuid(profile.transparentNotify) -> onNotify(Channel.TRANSPARENT, value)
            }
        }
    }
}
