package net.chelly.bankperfect.ui

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color

/** Couleurs propres aux montants, absentes du thème Material. */
data class Money(val negative: Color, val positive: Color)

val LocalMoney = staticCompositionLocalOf { Money(Color.Red, Color.Green) }

// Mêmes teintes que l'application de bureau (thèmes Tokyo Night).
private val Dark = darkColorScheme(
    primary = Color(0xFF7AA2F7),
    onPrimary = Color(0xFF1A1B26),
    secondary = Color(0xFFBB9AF7),
    background = Color(0xFF1A1B26),
    onBackground = Color(0xFFC0CAF5),
    surface = Color(0xFF1A1B26),
    onSurface = Color(0xFFC0CAF5),
    surfaceContainer = Color(0xFF24283B),
    surfaceContainerHigh = Color(0xFF292E42),
    onSurfaceVariant = Color(0xFF9AA5CE),
    outline = Color(0xFF414868),
    error = Color(0xFFF7768E),
)

private val Light = lightColorScheme(
    primary = Color(0xFF34548A),
    onPrimary = Color.White,
    secondary = Color(0xFF5A4A78),
    background = Color(0xFFF4F5F9),
    onBackground = Color(0xFF343B58),
    surface = Color(0xFFF4F5F9),
    onSurface = Color(0xFF343B58),
    surfaceContainer = Color.White,
    surfaceContainerHigh = Color(0xFFE9EBF2),
    onSurfaceVariant = Color(0xFF5F6587),
    outline = Color(0xFFC4C8DA),
    error = Color(0xFF8C4351),
)

@Composable
fun BankPerfectTheme(content: @Composable () -> Unit) {
    val dark = isSystemInDarkTheme()
    val money = if (dark) {
        Money(negative = Color(0xFFF7768E), positive = Color(0xFF9ECE6A))
    } else {
        Money(negative = Color(0xFF8C4351), positive = Color(0xFF33635C))
    }
    androidx.compose.runtime.CompositionLocalProvider(LocalMoney provides money) {
        MaterialTheme(colorScheme = if (dark) Dark else Light, content = content)
    }
}
