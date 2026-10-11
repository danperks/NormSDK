package io.github.danperks.normconsole

import android.os.Handler
import android.os.HandlerThread
import io.github.danperks.normsdk.Channel
import io.github.danperks.normsdk.OutgoingWrite
import io.github.danperks.normsdk.Priority
import io.github.danperks.normsdk.Request
import io.github.danperks.normsdk.Session
import io.github.danperks.normsdk.SessionEvent

/** Drives a NormSDK [Session] over a [WatchTransport] on one background thread. */
class NormLink(private val transport: WatchTransport) {
    val session = Session()
    var onEvent: (SessionEvent) -> Unit = {}
    var onWrite: (OutgoingWrite) -> Unit = {}

    private val thread = HandlerThread("normlink").apply { start() }
    private val handler = Handler(thread.looper)
    private val now get() = System.currentTimeMillis().toULong()

    private val pump = object : Runnable {
        override fun run() {
            for (write in session.pollTransmit(now)) {
                onWrite(write)
                transport.write(write.data, write.channel)
            }
            session.tick(now).forEach(onEvent)
            handler.postDelayed(this, 20)
        }
    }

    fun start() {
        transport.onNotify = { channel: Channel, data: ByteArray ->
            handler.post { session.handleNotification(channel, data, now).forEach(onEvent) }
        }
        transport.connect()
        session.setLinkReady(true)
        handler.post(pump)
    }

    fun send(request: Request, priority: Priority = Priority.NORMAL) {
        handler.post { session.enqueue(request, priority) }
    }

    fun stop() {
        handler.removeCallbacksAndMessages(null)
        session.clear()
        transport.disconnect()
        thread.quitSafely()
    }
}
