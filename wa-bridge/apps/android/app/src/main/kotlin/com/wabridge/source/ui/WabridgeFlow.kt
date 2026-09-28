package com.wabridge.source.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.PhoneAndroid
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.wabridge.source.ui.steps.ConnectStep
import com.wabridge.source.ui.steps.DestinationStatusStep
import com.wabridge.source.ui.steps.DirectionStep
import com.wabridge.source.ui.steps.ReadyCheckStep
import com.wabridge.source.ui.steps.WelcomeStep

/**
 * Responsive entry flow: Welcome -> Direction -> Destination -> Connect -> Ready.
 *
 * Fix for "off the mobile screen":
 * - BoxWithConstraints picks compact (<600dp) vs expanded (>=600dp) at runtime.
 * - Every screen scrolls + pads for system bars (insets from MainActivity).
 * - No hardcoded pixel sizes; CTA buttons always reachable on tall phones.
 * - All colors from MaterialTheme.colorScheme (light + dark work).
 */
@Composable
fun WabridgeFlow(
    screen: Int,
    onNext: () -> Unit,
    onBack: () -> Unit,
    insetsModifier: Modifier = Modifier,
) {
    BoxWithConstraints(
        modifier = insetsModifier.fillMaxSize(),
        contentAlignment = Alignment.TopCenter,
    ) {
        val expanded = maxWidth >= 600.dp
        val contentWidth = if (expanded) 560.dp else maxWidth
        Column(
            modifier = Modifier
                .widthIn(max = contentWidth)
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 20.dp, vertical = 16.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            StepProgress(current = screen, total = 5)
            when (screen) {
                0 -> WelcomeStep(expanded = expanded, onNext = onNext)
                1 -> DirectionStep(onNext = onNext, onBack = onBack)
                2 -> DestinationStatusStep(onNext = onNext, onBack = onBack)
                3 -> ConnectStep(onNext = onNext, onBack = onBack)
                else -> ReadyCheckStep(onNext = onNext, onBack = onBack)
            }
            Spacer(modifier = Modifier.height(8.dp))
        }
    }
}

@Composable
private fun StepProgress(current: Int, total: Int) {
    Column(
        modifier = Modifier.fillMaxWidth(),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Text(
            text = "${current + 1} / $total",
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        LinearProgressIndicator(
            progress = { (current + 1) / total.toFloat() },
            modifier = Modifier.fillMaxWidth().height(6.dp),
        )
    }
}


@Composable
internal fun HeroBadge() {
    androidx.compose.material3.Surface(
        tonalElevation = 2.dp,
        shape = MaterialTheme.shapes.extraLarge,
        color = MaterialTheme.colorScheme.primaryContainer,
    ) {
        Text(
            text = "WA",
            style = MaterialTheme.typography.headlineMedium,
            color = MaterialTheme.colorScheme.onPrimaryContainer,
            modifier = Modifier.padding(horizontal = 22.dp, vertical = 12.dp),
        )
    }
}

@Composable
internal fun OptionCard(title: String, body: String, onClick: () -> Unit) {
    androidx.compose.material3.Card(
        onClick = onClick,
        modifier = Modifier.fillMaxWidth(),
        colors = androidx.compose.material3.CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceContainerLow,
        ),
    ) {
        androidx.compose.foundation.layout.Row(
            modifier = Modifier.fillMaxWidth().padding(16.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(
                imageVector = Icons.Filled.PhoneAndroid,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.primary,
                modifier = Modifier.size(28.dp),
            )
            Spacer(modifier = Modifier.width(12.dp))
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = title,
                    style = MaterialTheme.typography.titleMedium,
                    color = MaterialTheme.colorScheme.onSurface,
                )
                Text(
                    text = body,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

@Composable
internal fun ChecklistCard(lines: List<String>) {
    androidx.compose.material3.Card(
        modifier = Modifier.fillMaxWidth(),
        colors = androidx.compose.material3.CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceContainerLow,
        ),
    ) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            lines.forEach { line ->
                androidx.compose.foundation.layout.Row(
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Icon(
                        imageVector = Icons.Filled.Check,
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.secondary,
                        modifier = Modifier.size(20.dp),
                    )
                    Spacer(modifier = Modifier.width(12.dp))
                    Text(
                        text = line,
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurface,
                    )
                }
            }
        }
    }
}

@Composable
internal fun NavRow(onNext: () -> Unit, onBack: () -> Unit, nextLabel: String? = null) {
    androidx.compose.foundation.layout.Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        androidx.compose.material3.OutlinedButton(
            onClick = onBack,
            modifier = Modifier.weight(1f).heightIn(min = 52.dp),
        ) {
            Text("Back")
        }
        androidx.compose.material3.Button(
            onClick = onNext,
            modifier = Modifier.weight(1f).heightIn(min = 52.dp),
        ) {
            Text(nextLabel ?: "Continue")
        }
    }
}

