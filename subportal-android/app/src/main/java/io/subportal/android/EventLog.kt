package io.subportal.android

import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import java.time.Instant
import java.util.concurrent.ConcurrentHashMap

/** Type of event recorded in the log. */
enum class EventType {
    OpenURI,
    OpenFile,
    Notify,
    Connected,
    Disconnected,
}

/** A single event recorded for a server. */
data class EventRecord(
    val timestamp: Instant,
    val type: EventType,
    val summary: String,
    val handled: Boolean,
)

/**
 * In-memory, per-server event log.
 *
 * Events are keyed by server name (the hostname received in callbacks).
 * The UI observes [updates] to reactively refresh when new events arrive.
 */
object EventLog {
    private const val MAX_EVENTS_PER_SERVER = 200

    private val store = ConcurrentHashMap<String, MutableList<EventRecord>>()

    private val _updates = MutableSharedFlow<String>(extraBufferCapacity = 8)

    /** Emits the server name whenever new events are recorded for it. */
    val updates: SharedFlow<String> = _updates.asSharedFlow()

    /** Record an event for [serverName]. Thread-safe. */
    fun record(serverName: String, type: EventType, summary: String, handled: Boolean = true) {
        val entry = EventRecord(
            timestamp = Instant.now(),
            type = type,
            summary = summary,
            handled = handled,
        )
        val list = store.getOrPut(serverName) { mutableListOf() }
        synchronized(list) {
            list.add(0, entry)
            if (list.size > MAX_EVENTS_PER_SERVER) {
                list.removeAt(list.lastIndex)
            }
        }
        _updates.tryEmit(serverName)
    }

    /** Return a snapshot of events for [serverName], newest first. */
    fun eventsFor(serverName: String): List<EventRecord> {
        val list = store[serverName] ?: return emptyList()
        synchronized(list) {
            return list.toList()
        }
    }

    /** Remove all events for [serverName]. */
    fun clear(serverName: String) {
        store.remove(serverName)
    }
}
