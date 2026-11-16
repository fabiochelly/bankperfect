package net.chelly.bankperfect.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** Carte arrondie aux couleurs Tokyo Night, bord fin. */
@Composable
fun TnCard(modifier: Modifier = Modifier, padding: Dp = 18.dp, content: @Composable ColumnScope.() -> Unit) {
    val t = LocalTokyo.current
    val shape = RoundedCornerShape(24.dp)
    Column(
        modifier
            .fillMaxWidth()
            .clip(shape)
            .background(t.card)
            .border(1.dp, t.border, shape)
            .padding(padding),
        content = content,
    )
}

/** Titre de section en petites capitales. */
@Composable
fun SectionTitle(text: String, modifier: Modifier = Modifier, trailing: @Composable () -> Unit = {}) {
    Row(modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Text(
            text.uppercase(),
            style = MaterialTheme.typography.labelMedium.copy(letterSpacing = 1.2.sp),
            color = LocalTokyo.current.faint,
            fontWeight = FontWeight.Bold,
            modifier = Modifier.weight(1f),
        )
        trailing()
    }
}

/** Pastille ronde avec une initiale, teintée. */
@Composable
fun Avatar(text: String, color: Color, size: Dp = 40.dp) {
    Box(
        Modifier
            .size(size)
            .clip(CircleShape)
            .background(color.copy(alpha = 0.18f)),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text.trim().firstOrNull()?.uppercase() ?: "•",
            color = color,
            fontWeight = FontWeight.Bold,
            style = MaterialTheme.typography.titleMedium,
        )
    }
}

/** Couleur stable d'un libellé (catégorie, compte). */
@Composable
fun colorFor(label: String): Color {
    val t = LocalTokyo.current
    return t.chartColor(label.hashCode().mod(t.chart.size))
}
