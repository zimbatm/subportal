package io.subportal.android.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.ClickableText
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import io.subportal.android.EventLog
import io.subportal.android.EventRecord
import io.subportal.android.EventType
import io.subportal.android.R
import io.subportal.android.SubportalCallbackImpl
import io.subportal.android.SubportalService
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.subportal_android_core.ServerInfo
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ServerDetailScreen(
    serverId: String,
    onBack: () -> Unit,
) {
    val scope = rememberCoroutineScope()
    var server by remember { mutableStateOf<ServerInfo?>(null) }
    var confirmingForget by remember { mutableStateOf(false) }
    var reconnecting by remember { mutableStateOf(false) }
    val events = remember { mutableStateListOf<EventRecord>() }

    fun refreshServer() {
        SubportalService.core?.let { core ->
            server = core.listServers().find { it.id == serverId }
        }
    }

    fun refreshEvents() {
        val name = server?.name ?: return
        events.clear()
        events.addAll(EventLog.eventsFor(name))
    }

    fun forgetServer() {
        scope.launch {
            withContext(Dispatchers.IO) {
                SubportalService.core?.forgetServer(serverId)
            }
            server?.name?.let { EventLog.clear(it) }
            onBack()
        }
    }

    fun reconnect() {
        scope.launch {
            reconnecting = true
            withContext(Dispatchers.IO) {
                SubportalService.core?.reconnect()
            }
            refreshServer()
            reconnecting = false
        }
    }

    // Initial load
    LaunchedEffect(serverId) {
        refreshServer()
        refreshEvents()
    }

    // Refresh on connection state changes
    LaunchedEffect(Unit) {
        SubportalCallbackImpl.serverUpdates.collect {
            refreshServer()
            refreshEvents()
        }
    }

    // Refresh on new events
    LaunchedEffect(server?.name) {
        val name = server?.name ?: return@LaunchedEffect
        EventLog.updates.collect { updatedName ->
            if (updatedName == name) {
                refreshEvents()
            }
        }
    }

    val currentServer = server

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(currentServer?.name ?: "") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(
                            Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = stringResource(R.string.detail_back),
                        )
                    }
                }
            )
        }
    ) { padding ->
        if (currentServer == null) {
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(padding),
                contentAlignment = Alignment.Center,
            ) {
                Text(stringResource(R.string.detail_not_found))
            }
            return@Scaffold
        }

        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(horizontal = 16.dp),
        ) {
            // -- Server info section --
            item {
                Spacer(modifier = Modifier.height(8.dp))
                ServerInfoSection(currentServer)
                Spacer(modifier = Modifier.height(16.dp))
                HorizontalDivider()
                Spacer(modifier = Modifier.height(16.dp))
            }

            // -- Event history header --
            item {
                Text(
                    text = stringResource(R.string.detail_event_history),
                    style = MaterialTheme.typography.titleMedium,
                )
                Spacer(modifier = Modifier.height(8.dp))
            }

            if (events.isEmpty()) {
                item {
                    Text(
                        text = stringResource(R.string.detail_no_events),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            } else {
                items(events) { event ->
                    EventRow(event)
                }
            }

            // -- Reconnect button --
            item {
                Spacer(modifier = Modifier.height(24.dp))
                HorizontalDivider()
                Spacer(modifier = Modifier.height(16.dp))
                Button(
                    onClick = { reconnect() },
                    modifier = Modifier.fillMaxWidth(),
                    enabled = !reconnecting,
                ) {
                    Text(
                        if (reconnecting) {
                            stringResource(R.string.detail_reconnecting)
                        } else {
                            stringResource(R.string.detail_reconnect)
                        }
                    )
                }
            }

            // -- Forget button --
            item {
                Spacer(modifier = Modifier.height(8.dp))
                if (confirmingForget) {
                    OutlinedButton(
                        onClick = { forgetServer() },
                        modifier = Modifier.fillMaxWidth(),
                        colors = ButtonDefaults.outlinedButtonColors(
                            contentColor = MaterialTheme.colorScheme.error,
                        ),
                    ) {
                        Text(stringResource(R.string.server_forget_confirm, currentServer.name))
                    }
                    TextButton(
                        onClick = { confirmingForget = false },
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Text(stringResource(R.string.detail_cancel))
                    }
                } else {
                    OutlinedButton(
                        onClick = { confirmingForget = true },
                        modifier = Modifier.fillMaxWidth(),
                        colors = ButtonDefaults.outlinedButtonColors(
                            contentColor = MaterialTheme.colorScheme.error,
                        ),
                    ) {
                        Text(stringResource(R.string.server_forget))
                    }
                }
                Spacer(modifier = Modifier.height(16.dp))
            }
        }
    }
}

@Composable
private fun ServerInfoSection(server: ServerInfo) {
    val dateFormatter = remember {
        DateTimeFormatter.ofLocalizedDateTime(FormatStyle.MEDIUM)
            .withZone(ZoneId.systemDefault())
    }

    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        // Connection status
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                text = stringResource(R.string.detail_status),
                style = MaterialTheme.typography.labelLarge,
                modifier = Modifier.weight(1f),
            )
            Text(
                text = if (server.connected) {
                    stringResource(R.string.server_connected)
                } else {
                    stringResource(R.string.server_disconnected)
                },
                color = if (server.connected) Color(0xFF4CAF50) else MaterialTheme.colorScheme.onSurfaceVariant,
                style = MaterialTheme.typography.bodyMedium,
            )
        }

        // Endpoint ID (selectable for copy)
        Text(
            text = stringResource(R.string.detail_endpoint_id),
            style = MaterialTheme.typography.labelLarge,
        )
        SelectionContainer {
            Text(
                text = server.id,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }

        // Enrolled date
        if (server.enrolledAt.isNotEmpty()) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    text = stringResource(R.string.detail_enrolled_at),
                    style = MaterialTheme.typography.labelLarge,
                    modifier = Modifier.weight(1f),
                )
                val formatted = try {
                    dateFormatter.format(Instant.parse(server.enrolledAt))
                } catch (_: Exception) {
                    server.enrolledAt
                }
                Text(
                    text = formatted,
                    style = MaterialTheme.typography.bodyMedium,
                )
            }
        }
    }
}

@Composable
private fun EventRow(event: EventRecord) {
    val timeFormatter = remember {
        DateTimeFormatter.ofLocalizedDateTime(FormatStyle.SHORT)
            .withZone(ZoneId.systemDefault())
    }

    val label = when (event.type) {
        EventType.OpenURI -> stringResource(R.string.event_open_uri)
        EventType.OpenFile -> stringResource(R.string.event_open_file)
        EventType.Notify -> stringResource(R.string.event_notify)
        EventType.Confirm -> stringResource(R.string.event_confirm)
        EventType.Connected -> stringResource(R.string.event_connected)
        EventType.Disconnected -> stringResource(R.string.event_disconnected)
    }

    val labelColor = when (event.type) {
        EventType.Connected -> Color(0xFF4CAF50)
        EventType.Disconnected -> MaterialTheme.colorScheme.error
        else -> MaterialTheme.colorScheme.primary
    }

    Column(modifier = Modifier.padding(vertical = 6.dp)) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = label,
                style = MaterialTheme.typography.labelMedium,
                color = labelColor,
            )
            Text(
                text = timeFormatter.format(event.timestamp),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        if (event.type != EventType.Connected && event.type != EventType.Disconnected) {
            if (event.type == EventType.OpenURI) {
                val uriHandler = LocalUriHandler.current
                val linkColor = MaterialTheme.colorScheme.primary
                val annotated = remember(event.summary) {
                    buildAnnotatedString {
                        pushStringAnnotation(tag = "URL", annotation = event.summary)
                        withStyle(SpanStyle(color = linkColor, textDecoration = TextDecoration.Underline)) {
                            append(event.summary)
                        }
                        pop()
                    }
                }
                @Suppress("DEPRECATION")
                ClickableText(
                    text = annotated,
                    style = MaterialTheme.typography.bodySmall,
                    maxLines = 2,
                    onClick = { offset ->
                        annotated.getStringAnnotations("URL", offset, offset)
                            .firstOrNull()?.let { uriHandler.openUri(it.item) }
                    },
                )
            } else {
                Text(
                    text = event.summary,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 2,
                )
            }
        }
        if (!event.handled) {
            Text(
                text = stringResource(R.string.event_not_handled),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.error,
            )
        }
    }
}
