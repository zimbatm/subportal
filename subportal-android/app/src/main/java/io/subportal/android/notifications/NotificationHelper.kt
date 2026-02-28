package io.subportal.android.notifications

import android.app.NotificationManager
import android.content.Context
import androidx.core.app.NotificationCompat
import java.util.concurrent.ConcurrentHashMap
import kotlin.math.abs

/**
 * Manages Android notifications mapped from subportal protocol notifications.
 */
object NotificationHelper {
    const val CHANNEL_SERVICE = "subportal_service"
    const val CHANNEL_NORMAL = "subportal_normal"
    const val CHANNEL_CRITICAL = "subportal_critical"

    const val SERVICE_NOTIFICATION_ID = 1

    /** Maps subportal notification IDs to Android notification IDs. */
    private val idMap = ConcurrentHashMap<String, Int>()
    private var nextId = 1000

    fun show(
        context: Context,
        notificationId: String,
        title: String,
        body: String?,
        urgency: String?,
        host: String
    ) {
        val channel = when (urgency) {
            "critical" -> CHANNEL_CRITICAL
            else -> CHANNEL_NORMAL
        }

        val androidId = synchronized(this) {
            nextId++.also { idMap[notificationId] = it }
        }

        val appName = if (host.isNotEmpty()) "subportal@$host" else "subportal"

        val notification = NotificationCompat.Builder(context, channel)
            .setContentTitle(title)
            .setContentText(body)
            .setSubText(appName)
            .setSmallIcon(android.R.drawable.ic_dialog_info)
            .setAutoCancel(true)
            .build()

        val manager = context.getSystemService(NotificationManager::class.java)
        manager.notify(androidId, notification)
    }

    fun dismiss(context: Context, id: String) {
        val androidId = idMap.remove(id) ?: return
        val manager = context.getSystemService(NotificationManager::class.java)
        manager.cancel(androidId)
    }
}
