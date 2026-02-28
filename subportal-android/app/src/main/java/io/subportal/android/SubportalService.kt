package io.subportal.android

import android.app.Service
import android.content.Intent
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat
import io.subportal.android.notifications.NotificationHelper
import uniffi.subportal_android_core.SubportalCore

/**
 * Foreground Service that owns the Rust SubportalCore instance.
 *
 * Keeps a persistent QUIC connection to enrolled servers and dispatches
 * incoming requests to the Android callback implementation.
 */
class SubportalService : Service() {

    private var core: SubportalCore? = null

    override fun onCreate() {
        super.onCreate()
        startForegroundWithNotification()

        val dataDir = filesDir.resolve("subportal").absolutePath
        val deviceName = Build.MODEL

        val callback = SubportalCallbackImpl(this)
        core = SubportalCore(dataDir, deviceName, callback)
        core?.start()
    }

    override fun onDestroy() {
        core?.stop()
        core = null
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        return START_STICKY
    }

    private fun startForegroundWithNotification() {
        val notification = NotificationCompat.Builder(this, NotificationHelper.CHANNEL_SERVICE)
            .setContentTitle(getString(R.string.service_notification_title))
            .setContentText(getString(R.string.service_notification_text, 0))
            .setSmallIcon(android.R.drawable.ic_menu_share)
            .setOngoing(true)
            .build()

        startForeground(NotificationHelper.SERVICE_NOTIFICATION_ID, notification)
    }
}
