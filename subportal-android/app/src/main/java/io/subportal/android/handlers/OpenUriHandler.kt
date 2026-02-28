package io.subportal.android.handlers

import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import io.subportal.android.R
import io.subportal.android.notifications.NotificationHelper
import java.util.concurrent.atomic.AtomicInteger

/**
 * Handles OpenURI requests.
 *
 * Android 10+ restricts launching activities from the background, so we post a
 * notification with a tap-to-open PendingIntent instead of calling
 * startActivity directly.
 */
object OpenUriHandler {
    private const val TAG = "OpenUriHandler"
    private val nextId = AtomicInteger(2000)

    fun open(context: Context, uri: String, host: String): Boolean {
        return try {
            val id = nextId.getAndIncrement()
            val viewIntent = Intent(Intent.ACTION_VIEW, Uri.parse(uri))
            val chooser = Intent.createChooser(viewIntent, uri).apply {
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            }
            val pendingIntent = PendingIntent.getActivity(
                context,
                id,
                chooser,
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
            )

            val notification = NotificationCompat.Builder(context, NotificationHelper.CHANNEL_NORMAL)
                .setContentTitle(context.getString(R.string.confirm_open_uri_title))
                .setContentText(uri)
                .setSubText("subportal@$host")
                .setSmallIcon(android.R.drawable.ic_menu_send)
                .setContentIntent(pendingIntent)
                .setAutoCancel(true)
                .build()

            val manager = NotificationManagerCompat.from(context)
            manager.notify(id, notification)
            true
        } catch (e: Exception) {
            Log.e(TAG, "Failed to open URI: $uri from $host", e)
            false
        }
    }
}
