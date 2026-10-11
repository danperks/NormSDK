package io.github.danperks.normconsole

import android.Manifest
import android.app.Activity
import android.graphics.Typeface
import android.os.Build
import android.os.Bundle
import android.view.ViewGroup.LayoutParams.MATCH_PARENT
import android.view.ViewGroup.LayoutParams.WRAP_CONTENT
import android.widget.ArrayAdapter
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ListView
import android.widget.ScrollView
import android.widget.TextView
import io.github.danperks.normsdk.CommandSpec
import io.github.danperks.normsdk.Request
import io.github.danperks.normsdk.SessionEvent
import io.github.danperks.normsdk.WriteChannel
import io.github.danperks.normsdk.commandCatalog
import io.github.danperks.normsdk.describeMessage
import io.github.danperks.normsdk.describeResponse
import io.github.danperks.normsdk.encodeFrame6e
import io.github.danperks.normsdk.encodeFrame6f
import io.github.danperks.normsdk.exampleRequest
import io.github.danperks.normsdk.FrameFormat
import io.github.danperks.normsdk.hexString
import io.github.danperks.normsdk.isDestructive

/** Reads a Norm Two's output and sends it catalog commands or raw frames. */
class MainActivity : Activity() {
    private var link: NormLink? = null
    private lateinit var status: TextView
    private lateinit var log: TextView
    private lateinit var logScroll: ScrollView
    private val catalog: List<CommandSpec> = commandCatalog()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val root = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setPadding(24, 24, 24, 24) }

        val host = EditText(this).apply { setText("10.0.2.2:7333"); hint = "bridge host:port" }
        val row = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL }
        row.addView(Button(this).apply {
            text = "Emulator"
            setOnClickListener {
                val (h, p) = host.text.toString().split(":").let { it[0] to (it.getOrNull(1)?.toIntOrNull() ?: 7333) }
                connect(BridgeTransport(h, p))
            }
        })
        row.addView(Button(this).apply {
            text = "Bluetooth"
            setOnClickListener {
                if (Build.VERSION.SDK_INT >= 31) {
                    requestPermissions(arrayOf(Manifest.permission.BLUETOOTH_SCAN, Manifest.permission.BLUETOOTH_CONNECT), 1)
                } else {
                    requestPermissions(arrayOf(Manifest.permission.ACCESS_FINE_LOCATION), 1)
                }
                connect(BleTransport(this@MainActivity))
            }
        })
        status = TextView(this).apply { text = "Not connected" }

        val raw = EditText(this).apply { hint = "6F 08 70 01 00 01 8F"; typeface = Typeface.MONOSPACE }
        val sendRaw = Button(this).apply {
            text = "Send raw"
            setOnClickListener {
                val bytes = raw.text.toString().filter { !it.isWhitespace() }.chunked(2).map { it.toInt(16).toByte() }
                link?.send(Request.Raw("raw", bytes.toByteArray(), WriteChannel.PRIMARY, null, 0u))
            }
        }

        val commands = ListView(this).apply {
            adapter = ArrayAdapter(this@MainActivity, android.R.layout.simple_list_item_2, android.R.id.text1, catalog.map { it.title })
            setOnItemClickListener { _, _, position, _ ->
                val spec = catalog[position]
                if (isDestructive(spec.kind)) append("· ${spec.id} is destructive; not sent from the sample")
                else link?.send(exampleRequest(spec.kind))
            }
        }
        log = TextView(this).apply { typeface = Typeface.MONOSPACE; textSize = 11f }
        logScroll = ScrollView(this).apply { addView(log) }

        root.addView(host, MATCH_PARENT, WRAP_CONTENT)
        root.addView(row)
        root.addView(status)
        root.addView(raw, MATCH_PARENT, WRAP_CONTENT)
        root.addView(sendRaw)
        root.addView(commands, LinearLayout.LayoutParams(MATCH_PARENT, 0, 1f))
        root.addView(logScroll, LinearLayout.LayoutParams(MATCH_PARENT, 0, 1f))
        setContentView(root)
    }

    private fun connect(transport: WatchTransport) {
        link?.stop()
        transport.onStatus = { text -> runOnUiThread { status.text = text } }
        link = NormLink(transport).apply {
            onWrite = { append("→ ${hexString(it.data)}  [${it.label}]") }
            onEvent = { event -> describe(event)?.let(::append) }
            start()
        }
    }

    private fun describe(event: SessionEvent): String? = when (event) {
        is SessionEvent.Received -> {
            val p = event.packet
            val frame = if (p.format == FrameFormat.FRAME6F) encodeFrame6f(p.command, p.action, p.payload)
            else encodeFrame6e(p.command, p.payload)
            "← ${hexString(frame)}\n   ${describeMessage(event.message)}"
        }
        is SessionEvent.Completed -> "= #${event.requestId} ${describeResponse(event.response)}"
        is SessionEvent.TimedOut -> "= #${event.requestId} timed out"
        is SessionEvent.Progress -> "· #${event.requestId} ${event.received}/${event.total}"
        is SessionEvent.Cancelled -> null
    }

    private fun append(line: String) = runOnUiThread {
        log.append(line + "\n")
        logScroll.post { logScroll.fullScroll(ScrollView.FOCUS_DOWN) }
    }

    override fun onDestroy() {
        link?.stop()
        super.onDestroy()
    }
}
