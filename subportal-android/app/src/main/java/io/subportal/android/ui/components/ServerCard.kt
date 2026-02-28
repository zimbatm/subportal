package io.subportal.android.ui.components

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Card
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import io.subportal.android.R
import uniffi.subportal_android_core.ServerInfo

@Composable
fun ServerCard(server: ServerInfo, onForget: (String) -> Unit) {
    var confirmingForget by remember { mutableStateOf(false) }

    Card(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp)
    ) {
        Row(
            modifier = Modifier.padding(16.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = server.name,
                    style = MaterialTheme.typography.titleMedium
                )
                Text(
                    text = server.id.take(16),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }

            Text(
                text = if (server.connected) {
                    stringResource(R.string.server_connected)
                } else {
                    stringResource(R.string.server_disconnected)
                },
                color = if (server.connected) {
                    Color(0xFF4CAF50)
                } else {
                    MaterialTheme.colorScheme.onSurfaceVariant
                },
                style = MaterialTheme.typography.labelMedium,
                modifier = Modifier.padding(end = 8.dp)
            )

            if (confirmingForget) {
                TextButton(onClick = {
                    onForget(server.id)
                    confirmingForget = false
                }) {
                    Text(
                        stringResource(R.string.server_forget_confirm, server.name),
                        color = MaterialTheme.colorScheme.error
                    )
                }
            } else {
                TextButton(onClick = { confirmingForget = true }) {
                    Text(stringResource(R.string.server_forget))
                }
            }
        }
    }
}
