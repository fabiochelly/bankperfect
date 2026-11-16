package net.chelly.bankperfect.ui

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/** Progression 0 → 1 rejouée quand `key` change (apparition des graphiques). */
@Composable
private fun reveal(key: Any?, millis: Int = 900): Float {
    val progress = remember { Animatable(0f) }
    LaunchedEffect(key) {
        progress.snapTo(0f)
        progress.animateTo(1f, tween(millis, easing = FastOutSlowInEasing))
    }
    return progress.value
}

/** Chemin lissé (courbes de Bézier) passant par les points. */
private fun smooth(points: List<Offset>): Path = Path().apply {
    if (points.isEmpty()) return@apply
    moveTo(points[0].x, points[0].y)
    for (i in 1 until points.size) {
        val p0 = points[i - 1]
        val p1 = points[i]
        val mid = (p0.x + p1.x) / 2
        cubicTo(mid, p0.y, mid, p1.y, p1.x, p1.y)
    }
}

/**
 * Courbe avec aire en dégradé, qui se dessine de gauche à droite.
 * `baseline` : ligne pointillée (zéro) si elle tombe dans le graphique.
 */
@Composable
fun AreaChart(
    values: List<Double>,
    color: Color,
    modifier: Modifier = Modifier,
    fillAlpha: Float = 0.35f,
    strokeWidth: Dp = 2.5.dp,
    baseline: Color? = null,
    compare: List<Double>? = null,
    compareColor: Color = Color.Gray,
) {
    val progress = reveal(values)
    Canvas(modifier) {
        val all = values + (compare ?: emptyList())
        if (values.size < 2 || all.isEmpty()) return@Canvas
        var min = all.min()
        var max = all.max()
        if (max - min < 1e-6) {
            max += 1
            min -= 1
        }
        val top = 6.dp.toPx()
        val bottom = 4.dp.toPx()
        val h = size.height - top - bottom
        val count = maxOf(values.size, compare?.size ?: 0)
        fun point(i: Int, v: Double) = Offset(
            x = size.width * i / (count - 1).coerceAtLeast(1),
            y = top + h * (1 - ((v - min) / (max - min))).toFloat(),
        )
        if (baseline != null && min < 0 && max > 0) {
            val y = point(0, 0.0).y
            drawLine(
                baseline,
                Offset(0f, y),
                Offset(size.width, y),
                strokeWidth = 1.dp.toPx(),
                pathEffect = PathEffect.dashPathEffect(floatArrayOf(8f, 8f)),
            )
        }
        compare?.takeIf { it.size >= 2 }?.let { other ->
            drawPath(
                smooth(other.mapIndexed(::point)),
                compareColor,
                style = Stroke(
                    width = 2.dp.toPx(),
                    cap = StrokeCap.Round,
                    pathEffect = PathEffect.dashPathEffect(floatArrayOf(10f, 10f)),
                ),
            )
        }
        val points = values.mapIndexed(::point)
        val line = smooth(points)
        val area = Path().apply {
            addPath(line)
            lineTo(points.last().x, size.height)
            lineTo(points.first().x, size.height)
            close()
        }
        clipRect(right = size.width * progress) {
            drawPath(area, Brush.verticalGradient(listOf(color.copy(alpha = fillAlpha), color.copy(alpha = 0f))))
            drawPath(line, color, style = Stroke(width = strokeWidth.toPx(), cap = StrokeCap.Round, join = StrokeJoin.Round))
        }
        if (progress > 0.98f) {
            val last = points.last()
            drawCircle(color.copy(alpha = 0.25f), radius = 7.dp.toPx(), center = last)
            drawCircle(color, radius = 3.5.dp.toPx(), center = last)
        }
    }
}

/** Anneau des parts (0 à 1), qui se remplit en tournant. */
@Composable
fun Donut(
    shares: List<Double>,
    colors: List<Color>,
    track: Color,
    modifier: Modifier = Modifier,
    size: Dp = 150.dp,
    thickness: Dp = 18.dp,
    center: @Composable () -> Unit = {},
) {
    val progress = reveal(shares, 1100)
    Box(modifier.size(size), contentAlignment = Alignment.Center) {
        Canvas(Modifier.size(size)) {
            val stroke = thickness.toPx()
            val inset = stroke / 2
            val arcSize = Size(this.size.width - stroke, this.size.height - stroke)
            drawArc(track, 0f, 360f, false, Offset(inset, inset), arcSize, style = Stroke(stroke))
            val gap = if (shares.count { it > 0.0 } > 1) 3f else 0f
            var start = -90f
            val total = 360f * progress
            shares.forEachIndexed { i, share ->
                val sweep = (share * 360).toFloat()
                val visible = (minOf(start + 90 + sweep, total) - (start + 90)).coerceAtLeast(0f)
                if (visible > gap) {
                    drawArc(
                        colors[i % colors.size],
                        start + gap / 2,
                        visible - gap,
                        false,
                        Offset(inset, inset),
                        arcSize,
                        style = Stroke(stroke, cap = StrokeCap.Butt),
                    )
                }
                start += sweep
            }
        }
        center()
    }
}

/** Barres jumelles recettes / dépenses par mois, qui montent. */
@Composable
fun MonthBars(
    income: List<Double>,
    expenses: List<Double>,
    incomeColor: Color,
    expenseColor: Color,
    highlight: Int,
    modifier: Modifier = Modifier,
) {
    val progress = reveal(income + expenses)
    Canvas(modifier) {
        val max = (income + expenses).maxOrNull()?.takeIf { it > 0 } ?: 1.0
        val n = income.size.coerceAtLeast(1)
        val slot = size.width / n
        val bar = (slot * 0.26f).coerceAtMost(14.dp.toPx())
        val gap = 3.dp.toPx()
        val radius = CornerRadius(bar / 2, bar / 2)
        for (i in 0 until n) {
            val alpha = if (i == highlight) 1f else 0.45f
            val cx = slot * i + slot / 2
            listOf(income[i] to incomeColor, expenses[i] to expenseColor).forEachIndexed { j, (v, c) ->
                val h = (size.height * (v / max) * progress).toFloat().coerceAtLeast(if (v > 0) 2f else 0f)
                val x = if (j == 0) cx - gap / 2 - bar else cx + gap / 2
                drawRoundRect(c.copy(alpha = alpha), Offset(x, size.height - h), Size(bar, h), radius)
            }
        }
    }
}

/** Barre de proportion arrondie (part d'une catégorie). */
@Composable
fun ShareBar(share: Double, color: Color, track: Color, modifier: Modifier = Modifier) {
    val progress = reveal(share, 700)
    Canvas(modifier) {
        val r = CornerRadius(size.height / 2, size.height / 2)
        drawRoundRect(track, cornerRadius = r)
        val w = (size.width * share * progress).toFloat()
        if (w > 0) drawRoundRect(color, size = Size(w.coerceAtLeast(size.height), size.height), cornerRadius = r)
    }
}
