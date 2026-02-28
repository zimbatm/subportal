package io.subportal.android.handlers

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.util.Log

/**
 * Handles OpenURI requests by launching ACTION_VIEW.
 */
object OpenUriHandler {
    private const val TAG = "OpenUriHandler"

    fun open(context: Context, uri: String, host: String): Boolean {
        return try {
            val intent = Intent(Intent.ACTION_VIEW, Uri.parse(uri)).apply {
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            }
            context.startActivity(intent)
            true
        } catch (e: Exception) {
            Log.e(TAG, "Failed to open URI: $uri from $host", e)
            false
        }
    }
}
