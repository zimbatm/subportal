package io.subportal.android.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.CloudOff
import androidx.compose.material.icons.filled.CloudQueue
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import io.subportal.android.R
import io.subportal.android.SubportalCallbackImpl
import io.subportal.android.SubportalService
import io.subportal.android.ui.components.ServerCard
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.subportal_android_core.ServerInfo

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ServerListScreen(onEnrollClick: () -> Unit, onServerClick: (String) -> Unit) {
    val scope = rememberCoroutineScope()
    val servers = remember { mutableStateListOf<ServerInfo>() }
    var coreRunning by remember { mutableStateOf(SubportalService.core != null) }

    fun refreshServers() {
        SubportalService.core?.let { core ->
            servers.clear()
            servers.addAll(core.listServers())
        }
        coreRunning = servers.any { it.connected }
    }

    // Initial load
    LaunchedEffect(Unit) {
        refreshServers()
        // Assume running if the core exists (it may not be connected yet)
        coreRunning = SubportalService.core != null
    }

    // Refresh when connection state changes
    LaunchedEffect(Unit) {
        SubportalCallbackImpl.serverUpdates.collect {
            refreshServers()
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.servers_title)) },
                actions = {
                    IconButton(onClick = {
                        scope.launch {
                            val core = SubportalService.core ?: return@launch
                            if (coreRunning) {
                                withContext(Dispatchers.IO) { core.stop() }
                                coreRunning = false
                            } else {
                                withContext(Dispatchers.IO) { core.start() }
                                coreRunning = true
                            }
                        }
                    }) {
                        if (coreRunning) {
                            Icon(
                                Icons.Default.CloudQueue,
                                contentDescription = stringResource(R.string.action_disconnect),
                            )
                        } else {
                            Icon(
                                Icons.Default.CloudOff,
                                contentDescription = stringResource(R.string.action_connect),
                            )
                        }
                    }
                },
            )
        },
        floatingActionButton = {
            FloatingActionButton(onClick = onEnrollClick) {
                Icon(Icons.Default.Add, contentDescription = stringResource(R.string.enroll_title))
            }
        }
    ) { padding ->
        if (servers.isEmpty()) {
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(padding),
                contentAlignment = Alignment.Center
            ) {
                Text(
                    text = stringResource(R.string.servers_empty),
                    style = MaterialTheme.typography.bodyLarge,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.padding(32.dp)
                )
            }
        } else {
            LazyColumn(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(padding),
                verticalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                items(servers) { server ->
                    ServerCard(
                        server = server,
                        onClick = { onServerClick(server.id) },
                    )
                }
            }
        }
    }
}
