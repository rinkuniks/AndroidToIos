package com.wabridge.source.ui.steps

import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.wabridge.source.R
import com.wabridge.source.ui.ChecklistCard
import com.wabridge.source.ui.NavRow

/** Screen 4 — Connect. */
@Composable
fun ConnectStep(onNext: () -> Unit, onBack: () -> Unit) {
    Text(
        text = stringResource(R.string.connect_title),
        style = MaterialTheme.typography.headlineSmall,
        color = MaterialTheme.colorScheme.onSurface,
        textAlign = TextAlign.Center,
    )
    Text(
        text = stringResource(R.string.connect_instructions),
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        textAlign = TextAlign.Center,
    )
    Text(
        text = stringResource(R.string.connect_waiting),
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    NavRow(onNext = onNext, onBack = onBack)
}

/** Screen 5 — Ready Check. */
@Composable
fun ReadyCheckStep(onNext: () -> Unit, onBack: () -> Unit) {
    Text(
        text = stringResource(R.string.ready_check_title),
        style = MaterialTheme.typography.headlineSmall,
        color = MaterialTheme.colorScheme.onSurface,
        textAlign = TextAlign.Center,
    )
    ChecklistCard(
        lines = listOf(
            stringResource(R.string.ready_check_wifi, "-"),
            stringResource(R.string.ready_check_battery, "-"),
            stringResource(R.string.ready_check_storage, "-"),
        ),
    )
    Spacer(modifier = Modifier.height(4.dp))
    Button(
        onClick = onNext,
        modifier = Modifier.fillMaxWidth().heightIn(min = 52.dp),
    ) {
        Text(stringResource(R.string.ready_check_start))
    }
    OutlinedButton(onClick = onBack, modifier = Modifier.fillMaxWidth()) {
        Text("Back")
    }
}
