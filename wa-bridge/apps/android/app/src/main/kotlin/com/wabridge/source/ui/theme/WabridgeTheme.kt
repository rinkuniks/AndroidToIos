package com.wabridge.source.ui.theme

import android.os.Build
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext

/**
 * Single source of truth for WA Bridge colors.
 *
 * Light + dark schemes are explicit (never pure white/black text tricks) so the
 * UI stays readable in both modes. Dynamic color is used on Android 12+ when
 * available, falling back to the brand palette otherwise.
 */
private val LightColors = lightColorScheme(
    primary = Color(0xFF0061A4),
    onPrimary = Color(0xFFFFFFFF),
    primaryContainer = Color(0xFFD1E4FF),
    onPrimaryContainer = Color(0xFF001D36),
    secondary = Color(0xFF008394),
    onSecondary = Color(0xFFFFFFFF),
    secondaryContainer = Color(0xFF9FF0FF),
    onSecondaryContainer = Color(0xFF001F25),
    surface = Color(0xFFFFF8F0),
    onSurface = Color(0xFF1C1B1F),
    surfaceVariant = Color(0xFFE3E2E6),
    onSurfaceVariant = Color(0xFF49454F),
    error = Color(0xFFBA1A20),
    onError = Color(0xFFFFFFFF),
    outline = Color(0xFF79747E),
    surfaceContainerLow = Color(0xFFF7F2FA),
)

private val DarkColors = darkColorScheme(
    primary = Color(0xFF90CAF9),
    onPrimary = Color(0xFF003366),
    primaryContainer = Color(0xFF00497D),
    onPrimaryContainer = Color(0xFFD1E4FF),
    secondary = Color(0xFF53D7F0),
    onSecondary = Color(0xFF003640),
    secondaryContainer = Color(0xFF004E5A),
    onSecondaryContainer = Color(0xFF9FF0FF),
    surface = Color(0xFF141218),
    onSurface = Color(0xFFE6E0E9),
    surfaceVariant = Color(0xFF49454F),
    onSurfaceVariant = Color(0xFFCAC4D0),
    error = Color(0xFFFFB4AB),
    onError = Color(0xFF690005),
    outline = Color(0xFF938F99),
    surfaceContainerLow = Color(0xFF1D1B20),
)

@Composable
fun WabridgeTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    dynamicColor: Boolean = true,
    content: @Composable () -> Unit,
) {
    val context = LocalContext.current
    val scheme = when {
        dynamicColor && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S -> {
            if (darkTheme) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
        }
        darkTheme -> DarkColors
        else -> LightColors
    }
    MaterialTheme(
        colorScheme = scheme,
        content = content,
    )
}
