package com.wabridge.source

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.consumeWindowInsets
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.systemBars
import androidx.compose.material3.Surface
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import com.wabridge.source.ui.WabridgeFlow
import com.wabridge.source.ui.theme.WabridgeTheme

/**
 * Main entry point: Welcome -> Direction -> Destination Status -> Connect -> Ready Check.
 *
 * Fixed for small/tall phones:
 * - enableEdgeToEdge() + systemBars padding so content never hides under the
 *   status bar, cutout, or gesture navigation bar.
 * - WabridgeFlow adapts to width (phones vs tablets) and scrolls, so the CTA
 *   is always reachable regardless of aspect ratio or font scale.
 * - WabridgeTheme follows the system dark/light setting automatically.
 */
class MainActivity : ComponentActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            WabridgeTheme {
                Surface {
                    var screen by rememberSaveable { mutableIntStateOf(0) }
                    val bars = WindowInsets.systemBars.asPaddingValues()
                    WabridgeFlow(
                        screen = screen,
                        onNext = { screen = (screen + 1).coerceAtMost(4) },
                        onBack = { screen = (screen - 1).coerceAtLeast(0) },
                        insetsModifier = Modifier
                            .padding(bars)
                            .consumeWindowInsets(bars),
                    )
                }
            }
        }
    }
}