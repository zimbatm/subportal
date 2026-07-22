package io.subportal.android.ui

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.ContextCompat
import androidx.navigation.compose.rememberNavController
import io.subportal.android.SubportalService
import io.subportal.android.ui.navigation.SubportalNavGraph
import io.subportal.android.ui.theme.SubportalTheme

class MainActivity : ComponentActivity() {

    private val notificationPermissionLauncher =
        registerForActivityResult(ActivityResultContracts.RequestPermission()) { _ -> }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        requestNotificationPermission()
        startSubportalService(intent?.getStringExtra(SubportalService.EXTRA_ENROLL_TICKET))

        setContent {
            SubportalTheme {
                val navController = rememberNavController()
                SubportalNavGraph(navController = navController)
            }
        }
    }

    // Re-deliver an enrollment ticket when the activity is already running.
    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        intent.getStringExtra(SubportalService.EXTRA_ENROLL_TICKET)?.let {
            startSubportalService(it)
        }
    }

    private fun requestNotificationPermission() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            if (ContextCompat.checkSelfPermission(
                    this,
                    Manifest.permission.POST_NOTIFICATIONS
                ) != PackageManager.PERMISSION_GRANTED
            ) {
                notificationPermissionLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
            }
        }
    }

    private fun startSubportalService(enrollTicket: String? = null) {
        val intent = Intent(this, SubportalService::class.java)
        if (enrollTicket != null) {
            intent.putExtra(SubportalService.EXTRA_ENROLL_TICKET, enrollTicket)
        }
        ContextCompat.startForegroundService(this, intent)
    }
}
