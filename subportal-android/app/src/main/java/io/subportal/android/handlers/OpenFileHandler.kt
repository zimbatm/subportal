package io.subportal.android.handlers

import android.content.Context
import android.content.Intent
import android.util.Base64
import android.util.Log
import androidx.core.content.FileProvider
import java.io.File

/**
 * Handles OpenFile requests by saving to cache and launching ACTION_VIEW.
 */
object OpenFileHandler {
    private const val TAG = "OpenFileHandler"

    fun open(
        context: Context,
        name: String,
        mime: String,
        contentBase64: String,
        host: String
    ): Boolean {
        return try {
            val data = Base64.decode(contentBase64, Base64.DEFAULT)

            val cacheDir = File(context.cacheDir, "subportal")
            cacheDir.mkdirs()

            val file = File(cacheDir, name)
            file.writeBytes(data)

            val uri = FileProvider.getUriForFile(
                context,
                "${context.packageName}.fileprovider",
                file
            )

            val intent = Intent(Intent.ACTION_VIEW).apply {
                setDataAndType(uri, mime)
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            }
            context.startActivity(intent)
            true
        } catch (e: Exception) {
            Log.e(TAG, "Failed to open file: $name from $host", e)
            false
        }
    }
}
