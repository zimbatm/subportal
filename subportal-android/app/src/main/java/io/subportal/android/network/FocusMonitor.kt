package io.subportal.android.network

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.os.PowerManager
import android.util.Log
import uniffi.subportal_android_core.SubportalCore

/**
 * Monitors screen on/off events and reports focus state to SubportalCore.
 *
 * When the screen is off or the device is not interactive, the focus state
 * is set to Idle so the server can prefer active devices for routing.
 */
class FocusMonitor(
    private val context: Context,
    private val core: SubportalCore
) {
    companion object {
        private const val TAG = "FocusMonitor"
    }

    private val powerManager =
        context.getSystemService(Context.POWER_SERVICE) as PowerManager

    private val receiver = object : BroadcastReceiver() {
        override fun onReceive(ctx: Context, intent: Intent) {
            when (intent.action) {
                Intent.ACTION_SCREEN_ON -> {
                    Log.d(TAG, "Screen on")
                    core.setFocusActive(true)
                }
                Intent.ACTION_SCREEN_OFF -> {
                    Log.d(TAG, "Screen off")
                    core.setFocusActive(false)
                }
            }
        }
    }

    fun start() {
        // Set initial state
        core.setFocusActive(powerManager.isInteractive)

        val filter = IntentFilter().apply {
            addAction(Intent.ACTION_SCREEN_ON)
            addAction(Intent.ACTION_SCREEN_OFF)
        }
        context.registerReceiver(receiver, filter)
    }

    fun stop() {
        context.unregisterReceiver(receiver)
    }
}
