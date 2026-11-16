package net.chelly.bankperfect.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import net.chelly.bankperfect.Model
import net.chelly.bankperfect.core.CategoryShare
import net.chelly.bankperfect.core.MonthReport

/** Analyse d'un mois : synthèse, six mois, rythme des dépenses, catégories. */
@Composable
fun Analysis(model: Model, padding: PaddingValues) {
    val report = model.report ?: return
    var incomeView by rememberSaveable { mutableStateOf(false) }
    LazyColumn(
        contentPadding = PaddingValues(
            start = 16.dp,
            end = 16.dp,
            top = padding.calculateTopPadding() + 8.dp,
            bottom = padding.calculateBottomPadding() + 24.dp,
        ),
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        item {
            MonthSwitcher(report.label, canGoForward = model.reportOffset < 0) { model.changeReportOffset(it) }
        }
        item { Summary(report) }
        item { SixMonths(report) }
        if (report.cumulative.size > 1) item { Rhythm(report) }
        item {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                FilterChip(selected = !incomeView, onClick = { incomeView = false }, label = { Text("Dépenses") })
                FilterChip(selected = incomeView, onClick = { incomeView = true }, label = { Text("Recettes") })
            }
        }
        val categories = if (incomeView) report.incomeCategories else report.expenseCategories
        if (categories.isEmpty()) {
            item {
                Text(
                    "Rien ce mois-ci",
                    color = LocalTokyo.current.faint,
                    modifier = Modifier.padding(16.dp),
                )
            }
        }
        categories.forEachIndexed { i, category ->
            item(key = "${incomeView}_${category.label}") { CategoryRow(category, LocalTokyo.current.chartColor(i)) }
        }
    }
}

@Composable
private fun MonthSwitcher(label: String, canGoForward: Boolean, onMove: (Int) -> Unit) {
    val t = LocalTokyo.current
    Row(Modifier.fillMaxWidth().padding(top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
        IconButton(onClick = { onMove(-1) }) { Icon(Icons.ChevronLeft, contentDescription = "Mois précédent") }
        Text(
            label.replaceFirstChar { it.uppercase() },
            style = MaterialTheme.typography.headlineSmall,
            modifier = Modifier.weight(1f),
            textAlign = androidx.compose.ui.text.style.TextAlign.Center,
        )
        IconButton(onClick = { onMove(1) }, enabled = canGoForward) {
            Icon(Icons.ChevronRight, contentDescription = "Mois suivant", tint = if (canGoForward) t.text else t.faint.copy(alpha = 0.4f))
        }
    }
}

@Composable
private fun Summary(report: MonthReport) = Column {
    val t = LocalTokyo.current
    Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        Tile("Recettes", report.income, t.green, Modifier.weight(1f))
        Tile("Dépenses", report.expenses, t.red, Modifier.weight(1f))
    }
    Spacer(Modifier.height(10.dp))
    TnCard {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text("Épargne du mois", color = t.muted, style = MaterialTheme.typography.labelLarge)
                Text(
                    (if (!report.netNegative) "+" else "") + report.net,
                    style = MaterialTheme.typography.headlineSmall.merge(Figures),
                    color = if (report.netNegative) t.red else t.text,
                )
            }
            val rate = report.savingsRate.coerceIn(0.0, 1.0)
            Donut(
                shares = listOf(rate),
                colors = listOf(if (report.netNegative) t.red else t.teal),
                track = t.cardHigh,
                size = 64.dp,
                thickness = 8.dp,
            ) {
                Text(
                    "${(report.savingsRate * 100).toInt()} %",
                    style = MaterialTheme.typography.labelMedium.merge(Figures),
                    color = t.text,
                )
            }
        }
    }
}

@Composable
private fun Tile(label: String, value: String, color: Color, modifier: Modifier) {
    val t = LocalTokyo.current
    TnCard(modifier, padding = 16.dp) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.size(8.dp).clip(CircleShape).background(color))
            Spacer(Modifier.width(6.dp))
            Text(label, color = t.muted, style = MaterialTheme.typography.labelLarge)
        }
        Spacer(Modifier.height(4.dp))
        Text(value, style = MaterialTheme.typography.titleLarge.merge(Figures), maxLines = 1)
    }
}

@Composable
private fun SixMonths(report: MonthReport) {
    val t = LocalTokyo.current
    TnCard {
        SectionTitle("Six derniers mois")
        Spacer(Modifier.height(14.dp))
        MonthBars(
            income = report.months.map { it.income },
            expenses = report.months.map { it.expenses },
            incomeColor = t.green,
            expenseColor = t.red,
            highlight = report.months.size - 1,
            modifier = Modifier.fillMaxWidth().height(130.dp),
        )
        Spacer(Modifier.height(6.dp))
        Row {
            report.months.forEachIndexed { i, m ->
                Text(
                    m.label,
                    style = MaterialTheme.typography.labelSmall,
                    color = if (i == report.months.size - 1) t.text else t.faint,
                    textAlign = androidx.compose.ui.text.style.TextAlign.Center,
                    modifier = Modifier.weight(1f),
                )
            }
        }
    }
}

/** Dépenses cumulées du mois, comparées au mois précédent (pointillés). */
@Composable
private fun Rhythm(report: MonthReport) {
    val t = LocalTokyo.current
    TnCard {
        SectionTitle("Rythme des dépenses")
        Spacer(Modifier.height(4.dp))
        Row(verticalAlignment = Alignment.CenterVertically) {
            Legend("Ce mois-ci", t.purple, dashed = false)
            Spacer(Modifier.width(16.dp))
            Legend("Mois précédent", t.faint, dashed = true)
        }
        Spacer(Modifier.height(12.dp))
        AreaChart(
            values = report.cumulative.map { it.value },
            compare = report.previousCumulative.map { it.value },
            compareColor = t.faint,
            color = t.purple,
            modifier = Modifier.fillMaxWidth().height(140.dp),
        )
    }
}

@Composable
private fun Legend(label: String, color: Color, dashed: Boolean) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Box(
            Modifier
                .width(14.dp)
                .height(if (dashed) 2.dp else 3.dp)
                .clip(RoundedCornerShape(2.dp))
                .background(color),
        )
        Spacer(Modifier.width(6.dp))
        Text(label, style = MaterialTheme.typography.bodySmall, color = LocalTokyo.current.muted)
    }
}

/** Catégorie : montant, part, barre ; un appui déplie les sous-catégories. */
@Composable
private fun CategoryRow(category: CategoryShare, color: Color) {
    val t = LocalTokyo.current
    var open by remember { mutableStateOf(false) }
    val expandable = category.children.size > 1 ||
        (category.children.size == 1 && category.children[0].label != "Sans sous-catégorie")
    TnCard(Modifier.clickable(enabled = expandable) { open = !open }, padding = 14.dp) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Avatar(category.label, color, size = 38.dp)
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        category.label,
                        style = MaterialTheme.typography.bodyLarge,
                        fontWeight = FontWeight.SemiBold,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.weight(1f),
                    )
                    Text(category.amount, style = MaterialTheme.typography.bodyLarge.merge(Figures), fontWeight = FontWeight.SemiBold)
                }
                Spacer(Modifier.height(6.dp))
                Row(verticalAlignment = Alignment.CenterVertically) {
                    ShareBar(category.share, color, t.cardHigh, Modifier.weight(1f).height(6.dp))
                    Spacer(Modifier.width(10.dp))
                    Text(
                        "${(category.share * 100).toInt()} %",
                        style = MaterialTheme.typography.labelMedium.merge(Figures),
                        color = t.muted,
                    )
                    if (expandable) {
                        Icon(
                            Icons.ChevronDown,
                            contentDescription = null,
                            tint = t.faint,
                            modifier = Modifier.size(18.dp).rotate(if (open) 180f else 0f),
                        )
                    }
                }
            }
        }
        AnimatedVisibility(open) {
            Column(Modifier.padding(start = 50.dp, top = 8.dp)) {
                category.children.forEach { child ->
                    Row(Modifier.fillMaxWidth().padding(vertical = 5.dp), verticalAlignment = Alignment.CenterVertically) {
                        Text(
                            child.label,
                            style = MaterialTheme.typography.bodyMedium,
                            color = t.muted,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.weight(1f),
                        )
                        Text(child.amount, style = MaterialTheme.typography.bodyMedium.merge(Figures))
                    }
                    ShareBar(child.share, color.copy(alpha = 0.6f), t.cardHigh, Modifier.fillMaxWidth().height(3.dp))
                }
            }
        }
    }
}
