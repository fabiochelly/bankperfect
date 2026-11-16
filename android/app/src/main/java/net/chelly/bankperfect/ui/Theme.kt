package net.chelly.bankperfect.ui

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * Couleurs Tokyo Night (« Night » en sombre, « Day » en clair), les mêmes
 * que l'application de bureau, plus ce que Material n'a pas : montants,
 * dégradé de la carte principale, couleurs des graphiques.
 */
data class Tokyo(
    val dark: Boolean,
    /** Fond de page, un cran plus sombre que les cartes. */
    val page: Color,
    val card: Color,
    val cardHigh: Color,
    val border: Color,
    val text: Color,
    val muted: Color,
    val faint: Color,
    val blue: Color,
    val purple: Color,
    val cyan: Color,
    val green: Color,
    val red: Color,
    val orange: Color,
    val yellow: Color,
    val teal: Color,
    /** Texte posé sur le dégradé de la carte principale. */
    val onHero: Color,
) {
    val hero: Brush get() = Brush.linearGradient(listOf(blue, purple))

    /** Couleurs des séries et catégories, dans l'ordre. */
    val chart: List<Color> get() = listOf(blue, purple, cyan, orange, green, red, yellow, teal)

    fun chartColor(index: Int): Color = chart[index % chart.size]
}

private val Night = Tokyo(
    dark = true,
    page = Color(0xFF16161E),
    card = Color(0xFF1F2335),
    cardHigh = Color(0xFF292E42),
    border = Color(0xFF292E42),
    text = Color(0xFFC0CAF5),
    muted = Color(0xFF9AA5CE),
    faint = Color(0xFF565F89),
    blue = Color(0xFF7AA2F7),
    purple = Color(0xFFBB9AF7),
    cyan = Color(0xFF7DCFFF),
    green = Color(0xFF9ECE6A),
    red = Color(0xFFF7768E),
    orange = Color(0xFFFF9E64),
    yellow = Color(0xFFE0AF68),
    teal = Color(0xFF73DACA),
    onHero = Color(0xFF16161E),
)

private val Day = Tokyo(
    dark = false,
    page = Color(0xFFE1E2E7),
    card = Color(0xFFF2F2F6),
    cardHigh = Color(0xFFE9E9EE),
    border = Color(0xFFD5D6DB),
    text = Color(0xFF343B58),
    muted = Color(0xFF5A607D),
    faint = Color(0xFF8990B3),
    blue = Color(0xFF2E7DE9),
    purple = Color(0xFF9854F1),
    cyan = Color(0xFF007197),
    green = Color(0xFF587539),
    red = Color(0xFFF52A65),
    orange = Color(0xFFB15C00),
    yellow = Color(0xFF8C6C3E),
    teal = Color(0xFF118C74),
    onHero = Color.White,
)

val LocalTokyo = staticCompositionLocalOf { Night }

/** Couleurs propres aux montants. */
data class Money(val negative: Color, val positive: Color)

val LocalMoney = staticCompositionLocalOf { Money(Color.Red, Color.Green) }

/** Chiffres à chasse fixe : les montants s'alignent et ne bougent pas. */
val Figures = TextStyle(fontFeatureSettings = "tnum")

private fun scheme(t: Tokyo) = if (t.dark) {
    darkColorScheme(
        primary = t.blue,
        onPrimary = t.page,
        primaryContainer = t.cardHigh,
        onPrimaryContainer = t.blue,
        secondary = t.purple,
        onSecondary = t.page,
        secondaryContainer = Color(0xFF2E2A4A),
        onSecondaryContainer = t.purple,
        tertiary = t.cyan,
        background = t.page,
        onBackground = t.text,
        surface = t.page,
        onSurface = t.text,
        surfaceContainerLowest = t.page,
        surfaceContainerLow = t.card,
        surfaceContainer = t.card,
        surfaceContainerHigh = t.cardHigh,
        surfaceContainerHighest = Color(0xFF343A55),
        surfaceVariant = t.cardHigh,
        onSurfaceVariant = t.muted,
        outline = Color(0xFF3B4261),
        outlineVariant = t.border,
        error = t.red,
        inverseSurface = t.text,
        inverseOnSurface = t.page,
    )
} else {
    lightColorScheme(
        primary = t.blue,
        onPrimary = Color.White,
        primaryContainer = Color(0xFFD6E2F8),
        onPrimaryContainer = Color(0xFF1D4E9E),
        secondary = t.purple,
        onSecondary = Color.White,
        secondaryContainer = Color(0xFFE6DAFB),
        onSecondaryContainer = Color(0xFF5A2E9E),
        tertiary = t.cyan,
        background = t.page,
        onBackground = t.text,
        surface = t.page,
        onSurface = t.text,
        surfaceContainerLowest = Color.White,
        surfaceContainerLow = t.card,
        surfaceContainer = t.card,
        surfaceContainerHigh = t.cardHigh,
        surfaceContainerHighest = Color(0xFFDCDDE3),
        surfaceVariant = t.cardHigh,
        onSurfaceVariant = t.muted,
        outline = Color(0xFFB4B6C4),
        outlineVariant = t.border,
        error = t.red,
    )
}

private val shapes = Shapes(
    extraSmall = RoundedCornerShape(8.dp),
    small = RoundedCornerShape(12.dp),
    medium = RoundedCornerShape(18.dp),
    large = RoundedCornerShape(24.dp),
    extraLarge = RoundedCornerShape(32.dp),
)

private fun typography(): Typography {
    val base = Typography()
    return base.copy(
        displaySmall = base.displaySmall.copy(fontWeight = FontWeight.SemiBold, letterSpacing = (-0.5).sp),
        headlineMedium = base.headlineMedium.copy(fontWeight = FontWeight.SemiBold, letterSpacing = (-0.3).sp),
        headlineSmall = base.headlineSmall.copy(fontWeight = FontWeight.SemiBold),
        titleLarge = base.titleLarge.copy(fontWeight = FontWeight.SemiBold),
        titleMedium = base.titleMedium.copy(fontWeight = FontWeight.SemiBold),
        labelLarge = base.labelLarge.copy(fontWeight = FontWeight.SemiBold),
    )
}

@Composable
fun BankPerfectTheme(content: @Composable () -> Unit) {
    val tokyo = if (isSystemInDarkTheme()) Night else Day
    val money = Money(negative = tokyo.red, positive = tokyo.green)
    CompositionLocalProvider(LocalTokyo provides tokyo, LocalMoney provides money) {
        MaterialTheme(colorScheme = scheme(tokyo), shapes = shapes, typography = typography(), content = content)
    }
}
