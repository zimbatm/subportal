package io.subportal.android

import android.app.Application
import android.app.NotificationChannel
import android.app.NotificationManager
import io.subportal.android.notifications.NotificationHelper

class SubportalApp : Application() {
    override fun onCreate() {
        super.onCreate()
        createNotificationChannels()
    }

    private fun createNotificationChannels() {
        val manager = getSystemService(NotificationManager::class.java)

        val serviceChannel = NotificationChannel(
            NotificationHelper.CHANNEL_SERVICE,
            getString(R.string.channel_service),
            NotificationManager.IMPORTANCE_LOW
        ).apply {
            description = "Persistent notification for the subportal background service"
        }

        val normalChannel = NotificationChannel(
            NotificationHelper.CHANNEL_NORMAL,
            getString(R.string.channel_normal),
            NotificationManager.IMPORTANCE_DEFAULT
        ).apply {
            description = "Notifications forwarded from enrolled servers"
        }

        val criticalChannel = NotificationChannel(
            NotificationHelper.CHANNEL_CRITICAL,
            getString(R.string.channel_critical),
            NotificationManager.IMPORTANCE_HIGH
        ).apply {
            description = "Critical notifications forwarded from enrolled servers"
        }

        manager.createNotificationChannels(listOf(serviceChannel, normalChannel, criticalChannel))
    }
}
