package io.subportal.android.notifications

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent

/** Receives Approve/Deny taps from confirm notifications and unblocks the waiter. */
class ConfirmActionReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != ConfirmManager.ACTION_DECISION) return
        val id = intent.getStringExtra(ConfirmManager.EXTRA_ID) ?: return
        ConfirmManager.submit(id, intent.getBooleanExtra(ConfirmManager.EXTRA_APPROVED, false))
    }
}
