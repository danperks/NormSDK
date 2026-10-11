package io.github.danperks.normsdk

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class NormSdkTest {
    private fun bytes(vararg values: Int) = ByteArray(values.size) { values[it].toByte() }

    @Test
    fun encodesRequests() {
        val frames = encodeRequest(Request.GetBattery(newMode = true))
        assertEquals(1, frames.size)
        assertArrayEquals(bytes(0x6F, 0x08, 0x70, 0x01, 0x00, 0x01, 0x8F), frames[0].data)
        assertEquals(WriteChannel.PRIMARY, frames[0].channel)
        assertEquals(WriteChannel.REMOTE, encodeRequest(Request.FindPhoneStartAck)[0].channel)
    }

    @Test
    fun catalogIsAnnotated() {
        val catalog = commandCatalog()
        assertEquals(93, catalog.size)
        val battery = commandSpec(CommandKind.GET_BATTERY)
        assertEquals("battery.get", battery.id)
        assertEquals(FirmwareSupport.Stock, battery.firmware)
        assertEquals(FirmwareSupport.NoStockHandler, commandSpec(CommandKind.SET_LANGUAGE).firmware)
    }

    @Test
    fun sessionMatchesFragmentedReplies() {
        val session = Session()
        session.setLinkReady(true)
        val id = session.enqueue(Request.GetBattery(newMode = true), Priority.NORMAL)
        val writes = session.pollTransmit(0uL)
        assertEquals(1, writes.size)
        assertTrue(writes[0].awaitsResponse)

        val reply = encodeFrame6f(0x08u, Action.CheckResponse, bytes(0xCD))
        val events = chunkFrame(reply, 3u).flatMap { session.handleNotification(Channel.PRIMARY, it, 5uL) }
        val completed = events.last() as SessionEvent.Completed
        assertEquals(id, completed.requestId)
        assertEquals(Response.Battery(BatteryStatus(level = 77u, charging = true)), completed.response)
    }

    @Test
    fun acksTimeoutsAndEvents() {
        val session = Session()
        session.setLinkReady(true)
        val set = session.enqueue(Request.SetBrightness(3u), Priority.NORMAL)
        session.pollTransmit(0uL)
        val ack = encodeFrame6f(0x01u, Action.SetResponse, bytes(0x07, 0x00))
        val completed = session.handleNotification(Channel.PRIMARY, ack, 1uL).last()
        assertEquals(SessionEvent.Completed(set, Response.Ack(0x07u, 0u)), completed)

        val probe = session.enqueue(Request.ReadMemory(0x10000000u, 16u), Priority.NORMAL)
        session.pollTransmit(2uL)
        assertEquals(listOf(SessionEvent.TimedOut(probe, Response.Bytes(ByteArray(0)))).toString(),
            session.tick(2_002uL).toString())

        val packet = FrameDecoder().feed(Channel.PRIMARY, encodeFrame6f(0xD2u, Action.Set, bytes(0x00)))[0]
        assertEquals(WatchMessage.FindPhone(ringing = true), classifyPacket(packet))
    }
}
