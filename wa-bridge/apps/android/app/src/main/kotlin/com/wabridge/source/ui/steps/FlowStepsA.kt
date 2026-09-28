package com.wabridge.source.ui.steps

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.material3.Button
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.wabridge.source.R
import com.wabridge.source.ui.HeroBadge
import com.wabridge.source.ui.NavRow
import com.wabridge.source.ui.OptionCard

/** Screen 1 — Welcome. Two-pane row on tablets, stacked on phones. */
@Composable
fun WelcomeStep(expanded: Boolean, onNext: () -> Unit) {
    if (expanded) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(20.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            HeroBadge()
            Column(modifier = Modifier.weight(1f)) { WelcomeCopy() }
        }
    } else {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            HeroBadge()
            Spacer(modifier = Modifier.height(16.dp))
            WelcomeCopy()
        }
    }
    Spacer(modifier = Modifier.height(8.dp))
    Button(
        onClick = onNext,
        modifier = Modifier.fillMaxWidth().heightIn(min = 52.dp),
    ) {
        Text(stringResource(R.string.welcome_cta))
    }
    Text(
        text = stringResource(R.string.welcome_subtitle),
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        textAlign = TextAlign.Center,
    )
}

@Composable
private fun WelcomeCopy() {
    Text(
        text = stringResource(R.string.welcome_title),
        style = MaterialTheme.typography.headlineLarge,
        color = MaterialTheme.colorScheme.onSurface,
    )
    Spacer(modifier = Modifier.height(8.dp))
    Text(
        text = stringResource(R.string.welcome_subtitle),
        style = MaterialTheme.typography.bodyLarge,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    Spacer(modifier = Modifier.height(8.dp))
    Text(
        text = stringResource(R.string.vault_scanning),
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

/** Screen 2 — Direction. */
@Composable
fun DirectionStep(onNext: () -> Unit, onBack: () -> Unit) {
    Text(
        text = stringResource(R.string.direction_title),
        style = MaterialTheme.typography.headlineSmall,
        color = MaterialTheme.colorScheme.onSurface,
        textAlign = TextAlign.Center,
    )
    OptionCard(
        title = stringResource(R.string.direction_source),
        body = stringResource(R.string.welcome_subtitle),
        onClick = onNext,
    )
    OptionCard(
        title = stringResource(R.string.direction_destination),
        body = stringResource(R.string.connect_instructions),
        onClick = onNext,
    )
    NavRow(onNext = onNext, onBack = onBack)
}

/** Screen 3 — Destination Status. */
@Composable
fun DestinationStatusStep(onNext: () -> Unit, onBack: () -> Unit) {
    Text(
        text = stringResource(R.string.dest_status_title),
        style = MaterialTheme.typography.headlineSmall,
        color = MaterialTheme.colorScheme.onSurface,
        textAlign = TextAlign.Center,
    )
    Text(
        text = stringResource(R.string.dest_status_scanning),
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        textAlign = TextAlign.Center,
    )
    LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
    NavRow(
        onNext = onNext,
        onBack = onBack,
        nextLabel = stringResource(R.string.dest_status_retry),
    )
}
