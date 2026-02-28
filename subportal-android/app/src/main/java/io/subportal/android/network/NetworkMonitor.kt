package io.subportal.android.network

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.util.Log
import uniffi.subportal_android_core.SubportalCore

/**
 * Monitors network connectivity changes and notifies SubportalCore
 * so the iroh endpoint can adapt (e.g. WiFi <-> cellular transitions).
 */
class NetworkMonitor(
    private val context: Context,
    private val core: SubportalCore
) {
    companion object {
        private const val TAG = "NetworkMonitor"
    }

    private val connectivityManager =
        context.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager

    private val callback = object : ConnectivityManager.NetworkCallback() {
        override fun onAvailable(network: Network) {
            Log.d(TAG, "Network available")
            core.networkChanged()
        }

        override fun onLost(network: Network) {
            Log.d(TAG, "Network lost")
            core.networkChanged()
        }

        override fun onCapabilitiesChanged(
            network: Network,
            capabilities: NetworkCapabilities
        ) {
            Log.d(TAG, "Network capabilities changed")
            core.networkChanged()
        }
    }

    fun start() {
        val request = NetworkRequest.Builder()
            .addCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
            .build()
        connectivityManager.registerNetworkCallback(request, callback)
    }

    fun stop() {
        connectivityManager.unregisterNetworkCallback(callback)
    }
}
