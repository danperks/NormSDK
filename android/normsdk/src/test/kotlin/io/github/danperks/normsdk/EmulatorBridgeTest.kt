package io.github.danperks.normsdk

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assume.assumeTrue
import org.junit.Test
import java.io.BufferedReader
import java.io.InputStreamReader
import java.net.InetSocketAddress
import java.net.Socket

/**
 * Talks to the emulated stock firmware through tools/emulator/bridge.py on
 * 127.0.0.1:7333. Skipped when no bridge is listening.
 */
class EmulatorBridgeTest {
    private fun connect(): Socket? = try {
        Socket().apply { connect(InetSocketAddress("127.0.0.1", 7333), 1000); soTimeout = 100 }
    } catch (e: Exception) {
        null
    }

    private fun hex(data: ByteArray) = data.joinToString("") { "%02X".format(it) }

    private fun call(socket: Socket, reader: BufferedReader, session: Session, request: Request): SessionEvent {
        val id = session.enqueue(request, Priority.NORMAL)
        val deadline = System.currentTimeMillis() + 5_000
        while (System.currentTimeMillis() < deadline) {
            val now = System.currentTimeMillis().toULong()
            for (write in session.pollTransmit(now)) {
                val line = JSONObject().put("type", "write").put("channel", "primary").put("data", hex(write.data))
                socket.getOutputStream().write((line.toString() + "\n").toByteArray())
            }
            val line = try { reader.readLine() } catch (e: java.net.SocketTimeoutException) { null } ?: continue
            val message = JSONObject(line)
            if (message.getString("type") != "notify") continue
            val data = message.getString("data").chunked(2).map { it.toInt(16).toByte() }.toByteArray()
            for (event in session.handleNotification(Channel.PRIMARY, data, now)) {
                if (event is SessionEvent.Completed && event.requestId == id) return event
            }
        }
        error("no reply to $request")
    }

    @Test
    fun readsAndWritesTheEmulatedWatch() {
        val socket = connect()
        assumeTrue("no emulator bridge on 127.0.0.1:7333", socket != null)
        socket!!.use {
            val reader = BufferedReader(InputStreamReader(it.getInputStream()))
            it.soTimeout = 2000
            assertEquals("hello", JSONObject(reader.readLine()).getString("type"))
            it.soTimeout = 100
            val session = Session().apply { setLinkReady(true) }

            val version = call(it, reader, session, Request.GetDeviceVersion) as SessionEvent.Completed
            assertEquals("A0.2R0.4T0.0H0.0B0.1", (version.response as Response.Version).text)

            val ack = call(it, reader, session, Request.SetScreenTimeout(15u)) as SessionEvent.Completed
            assertEquals(Response.Ack(0x13u, 0u), ack.response)
            val timeout = call(it, reader, session, Request.GetScreenTimeout) as SessionEvent.Completed
            assertEquals(Response.ScreenTimeout(15u), timeout.response)
        }
    }
}
