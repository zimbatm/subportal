package io.subportal.android.notifications

import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.util.Log
import androidx.core.app.NotificationCompat
import uniffi.subportal_android_core.ConfirmDecision
import java.util.concurrent.ArrayBlockingQueue
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger

/**
 * Handles Confirm requests: pop a heads-up notification with Approve/Deny and
 * block the calling (Rust) thread until the user answers.
 *
 * The server races the prompt across every Confirm-capable device; the first
 * user answer wins. A timeout is NO_DECISION, not a deny.
 */
object ConfirmManager {
    private const val TAG = "ConfirmManager"

    const val ACTION_DECISION = "io.subportal.android.CONFIRM_DECISION"
    const val EXTRA_ID = "confirm_id"
    const val EXTRA_APPROVED = "approved"

    private const val TIMEOUT_MS = 60_000L

    // Android notification IDs for confirm prompts, kept clear of NotificationHelper's range.
    private val nextAndroidId = AtomicInteger(9000)

    // A capacity-1 queue per in-flight confirm; an answer that arrives before we
    // start polling is buffered rather than lost.
    private val pending = ConcurrentHashMap<String, ArrayBlockingQueue<ConfirmDecision>>()

    /** Show the prompt and block until answered, dismissed, or timed out. */
    fun awaitDecision(
        context: Context,
        message: String,
        title: String?,
        host: String,
    ): ConfirmDecision {
        // Single atomic op so concurrent confirms can't share an id.
        val androidId = nextAndroidId.getAndIncrement()
        val id = "confirm-$androidId"
        val queue = ArrayBlockingQueue<ConfirmDecision>(1)
        pending[id] = queue
        try {
            show(context, androidId, id, message, title, host)
            val decision = queue.poll(TIMEOUT_MS, TimeUnit.MILLISECONDS)
            if (decision == null) {
                Log.i(TAG, "confirm $id timed out -> no decision")
                return ConfirmDecision.NO_DECISION
            }
            return decision
        } finally {
            pending.remove(id)
            context.getSystemService(NotificationManager::class.java).cancel(androidId)
        }
    }

    /** Deliver a user decision from [ConfirmActionReceiver]. */
    fun submit(id: String, approved: Boolean) {
        val decision = if (approved) ConfirmDecision.APPROVED else ConfirmDecision.DENIED
        pending[id]?.offer(decision) ?: Log.w(TAG, "decision for unknown confirm $id")
    }

    private fun show(
        context: Context,
        androidId: Int,
        id: String,
        message: String,
        title: String?,
        host: String,
    ) {
        val appName = if (host.isNotEmpty()) "subportal@$host" else "subportal"
        val notification = NotificationCompat.Builder(context, NotificationHelper.CHANNEL_CRITICAL)
            .setContentTitle(title ?: "Confirm")
            .setContentText(message)
            .setSubText(appName)
            .setSmallIcon(android.R.drawable.ic_dialog_alert)
            .setCategory(NotificationCompat.CATEGORY_CALL)
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .setAutoCancel(true)
            .addAction(0, "Approve", decisionIntent(context, androidId, id, true))
            .addAction(0, "Deny", decisionIntent(context, androidId, id, false))
            // Swiping the prompt away counts as a deny.
            .setDeleteIntent(decisionIntent(context, androidId, id, false))
            .build()

        context.getSystemService(NotificationManager::class.java).notify(androidId, notification)
    }

    private fun decisionIntent(
        context: Context,
        androidId: Int,
        id: String,
        approved: Boolean,
    ): PendingIntent {
        val intent = Intent(ACTION_DECISION)
            .setPackage(context.packageName)
            .putExtra(EXTRA_ID, id)
            .putExtra(EXTRA_APPROVED, approved)
        // Distinct request code per (prompt, decision) so the two actions don't collide.
        val requestCode = androidId * 2 + if (approved) 1 else 0
        return PendingIntent.getBroadcast(
            context,
            requestCode,
            intent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
    }
}
