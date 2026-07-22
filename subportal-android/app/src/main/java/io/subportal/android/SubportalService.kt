package io.subportal.android

import android.app.Service
import android.content.Intent
import android.content.pm.ApplicationInfo
import android.os.Build
import android.os.IBinder
import android.content.IntentFilter
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat
import io.subportal.android.network.FocusMonitor
import io.subportal.android.network.NetworkMonitor
import io.subportal.android.notifications.ConfirmActionReceiver
import io.subportal.android.notifications.ConfirmManager
import io.subportal.android.notifications.NotificationHelper
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import uniffi.subportal_android_core.SubportalCore

/**
 * Foreground Service that owns the Rust SubportalCore instance.
 *
 * Keeps a persistent QUIC connection to enrolled servers and dispatches
 * incoming requests to the Android callback implementation.
 */
class SubportalService : Service() {

    companion object {
        private const val TAG = "SubportalService"

        /** Accessible from the UI to call enroll / listServers. */
        var core: SubportalCore? = null
            private set

        /**
         * Intent extra carrying an enrollment ticket JSON. Lets a test harness
         * enroll over adb without the QR/paste UI:
         *   am start -n io.subportal.android/.ui.MainActivity --es ticket_json '<ticket>'
         * Honored on debuggable builds only.
         */
        const val EXTRA_ENROLL_TICKET = "ticket_json"
    }

    private val serviceScope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val confirmReceiver = ConfirmActionReceiver()
    private var networkMonitor: NetworkMonitor? = null
    private var focusMonitor: FocusMonitor? = null

    override fun onCreate() {
        super.onCreate()
        startForegroundWithNotification()

        ContextCompat.registerReceiver(
            this,
            confirmReceiver,
            IntentFilter(ConfirmManager.ACTION_DECISION),
            ContextCompat.RECEIVER_NOT_EXPORTED,
        )

        val dataDir = filesDir.resolve("subportal").absolutePath
        val deviceName = Build.MODEL

        val callback = SubportalCallbackImpl(this)
        val c = SubportalCore(dataDir, deviceName, callback)
        core = c
        c.start()

        networkMonitor = NetworkMonitor(this, c).also { it.start() }
        focusMonitor = FocusMonitor(this, c).also { it.start() }
    }

    override fun onDestroy() {
        serviceScope.cancel()
        unregisterReceiver(confirmReceiver)
        focusMonitor?.stop()
        focusMonitor = null
        networkMonitor?.stop()
        networkMonitor = null
        core?.stop()
        core = null
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        intent?.getStringExtra(EXTRA_ENROLL_TICKET)?.let { enrollFromIntent(it) }
        return START_STICKY
    }

    // adb-drivable test enrollment. An exported enroll path is a footgun in a
    // shipped app, so refuse it unless this is a debuggable build.
    private fun enrollFromIntent(ticketJson: String) {
        if (applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE == 0) {
            Log.w(TAG, "ignoring $EXTRA_ENROLL_TICKET: not a debuggable build")
            return
        }
        val c = core
        if (c == null) {
            Log.w(TAG, "cannot enroll: core not ready")
            return
        }
        serviceScope.launch {
            try {
                Log.i(TAG, "enrolled with ${c.enroll(ticketJson)}")
            } catch (e: Exception) {
                Log.e(TAG, "enroll failed: ${e.message}")
            }
        }
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
