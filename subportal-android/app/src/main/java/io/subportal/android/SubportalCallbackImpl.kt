package io.subportal.android

import android.content.Context
import android.util.Log
import io.subportal.android.handlers.OpenFileHandler
import io.subportal.android.handlers.OpenUriHandler
import io.subportal.android.notifications.NotificationHelper
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import uniffi.subportal_android_core.SubportalCallback

/**
 * Bridges Rust callbacks to Android APIs.
 *
 * Each callback method is invoked from a Rust background thread. Methods that
 * need to show UI (e.g. confirmation dialogs) post to the main thread.
 */
class SubportalCallbackImpl(
    private val context: Context,
) : SubportalCallback {

    companion object {
        private const val TAG = "SubportalCallback"
        private val _serverUpdates = MutableSharedFlow<Unit>(extraBufferCapacity = 1)
        /** Emits whenever connection state changes so the UI can refresh. */
        val serverUpdates: SharedFlow<Unit> = _serverUpdates.asSharedFlow()
    }

    override fun onOpenUri(uri: String, host: String): Boolean {
        Log.i(TAG, "onOpenUri: $uri from $host")
        return OpenUriHandler.open(context, uri, host)
    }

    override fun onOpenFile(
        name: String,
        mime: String,
        contentBase64: String,
        host: String
    ): Boolean {
        Log.i(TAG, "onOpenFile: $name ($mime) from $host")
        return OpenFileHandler.open(context, name, mime, contentBase64, host)
    }

    override fun onNotify(
        notificationId: String,
        title: String,
        body: String?,
        urgency: String?,
        host: String
    ) {
        Log.i(TAG, "onNotify: $title from $host")
        NotificationHelper.show(context, notificationId, title, body, urgency, host)
    }

    override fun onDismissNotification(id: String) {
        Log.i(TAG, "onDismissNotification: $id")
        NotificationHelper.dismiss(context, id)
    }

    override fun onConnectionChanged(serverName: String, connected: Boolean) {
        Log.i(TAG, "onConnectionChanged: $serverName connected=$connected")
        _serverUpdates.tryEmit(Unit)
    }
}
