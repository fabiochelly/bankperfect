package net.chelly.bankperfect.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
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
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import java.time.LocalDate
import java.time.format.DateTimeFormatter
import java.util.Locale
import net.chelly.bankperfect.Model
import net.chelly.bankperfect.Tab
import net.chelly.bankperfect.Page
import net.chelly.bankperfect.core.AccountSummary
import net.chelly.bankperfect.core.Dashboard
import net.chelly.bankperfect.core.RecentItem
import net.chelly.bankperfect.core.UpcomingItem

/** Accueil : patrimoine, mois en cours, catégories, à venir, dernières opérations. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun Home(model: Model, padding: PaddingValues) {
    val dash = model.dashboard ?: return
    PullToRefreshBox(
        isRefreshing = model.refreshing,
        onRefresh = model::refresh,
        modifier = Modifier.fillMaxSize(),
    ) {
        LazyColumn(
            contentPadding = PaddingValues(
                start = 16.dp,
                end = 16.dp,
                top = padding.calculateTopPadding() + 8.dp,
                bottom = padding.calculateBottomPadding() + 96.dp,
            ),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            item { Greeting(model) }
            item { Hero(dash) }
            item { AccountStrip(model.accounts) { model.tab = Tab.ACCOUNTS; model.openAccount(it) } }
            item { MonthCard(dash) }
            model.budget?.let { budget -> item { BudgetCard(budget) } }
            if (dash.categories.isNotEmpty()) item { CategoriesCard(dash) { model.tab = Tab.ANALYSIS } }
            if (dash.upcoming.isNotEmpty()) {
                item { UpcomingCard(dash.upcoming, model.schedules.isNotEmpty()) { model.page = Page.SCHEDULES } }
            } else if (model.schedules.isNotEmpty()) {
                item { ScheduleLink(model) }
            }
            if (dash.recent.isNotEmpty()) item { RecentCard(dash.recent, model::editRecent) }
        }
    }
}

@Composable
private fun Greeting(model: Model) {
    val title = model.title
    val t = LocalTokyo.current
    val today = LocalDate.now().format(DateTimeFormatter.ofPattern("EEEE d MMMM", Locale.FRENCH))
    Row(Modifier.padding(top = 8.dp, bottom = 2.dp), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Text(today.replaceFirstChar { it.uppercase() }, color = t.faint, style = MaterialTheme.typography.labelLarge)
            Text(title, style = MaterialTheme.typography.headlineMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        SavingIndicator(model.saving)
        HideButton(model)
    }
}

/** Carte principale en dégradé : patrimoine, variation, courbe sur 90 jours. */
@Composable
private fun Hero(dash: Dashboard) {
    val t = LocalTokyo.current
    val on = t.onHero
    val shape = RoundedCornerShape(28.dp)
    Box(
        Modifier
            .fillMaxWidth()
            .clip(shape)
            .background(t.hero),
    ) {
        AreaChart(
            values = dash.history.map { it.value },
            color = on,
            fillAlpha = 0.18f,
            strokeWidth = 2.dp,
            modifier = Modifier
                .fillMaxWidth()
                .height(110.dp)
                .align(Alignment.BottomCenter)
                .padding(bottom = 64.dp),
        )
        Column(Modifier.padding(22.dp)) {
            Text("Patrimoine", color = on.copy(alpha = 0.75f), style = MaterialTheme.typography.labelLarge)
            Text(
                masked(dash.total),
                color = on,
                style = MaterialTheme.typography.displaySmall.merge(Figures),
                fontWeight = FontWeight.Bold,
            )
            Row(verticalAlignment = Alignment.CenterVertically) {
                Box(
                    Modifier
                        .clip(RoundedCornerShape(50))
                        .background(on.copy(alpha = 0.14f))
                        .padding(horizontal = 10.dp, vertical = 4.dp),
                ) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Icon(
                            if (dash.changeNegative) Icons.TrendDown else Icons.TrendUp,
                            contentDescription = null,
                            tint = on,
                            modifier = Modifier.size(16.dp),
                        )
                        Spacer(Modifier.width(4.dp))
                        Text(masked(dash.change30Days), color = on, style = MaterialTheme.typography.labelLarge.merge(Figures))
                    }
                }
                Spacer(Modifier.width(8.dp))
                Text("sur 30 jours", color = on.copy(alpha = 0.7f), style = MaterialTheme.typography.bodySmall)
            }
            Spacer(Modifier.height(96.dp))
            Row {
                HeroFigure("En banque", dash.pointed, on, Modifier.weight(1f))
                HeroFigure("Fin de mois prévue", dash.forecast, on, Modifier.weight(1f))
            }
        }
    }
}

@Composable
private fun HeroFigure(label: String, value: String, color: Color, modifier: Modifier) {
    Column(modifier) {
        Text(label, color = color.copy(alpha = 0.7f), style = MaterialTheme.typography.labelMedium)
        Text(masked(value), color = color, style = MaterialTheme.typography.titleMedium.merge(Figures))
    }
}

/** Comptes en bandeau horizontal. */
@Composable
private fun AccountStrip(accounts: List<AccountSummary>, onOpen: (Int) -> Unit) {
    val t = LocalTokyo.current
    LazyRow(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        items(accounts, key = { it.index.toInt() }) { account ->
            val accent = colorFor(account.name)
            val shape = RoundedCornerShape(20.dp)
            Column(
                Modifier
                    .width(168.dp)
                    .clip(shape)
                    .background(t.card)
                    .border(1.dp, t.border, shape)
                    .clickable { onOpen(account.index.toInt()) }
                    .padding(14.dp),
            ) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Box(
                        Modifier
                            .size(8.dp)
                            .clip(CircleShape)
                            .background(accent),
                    )
                    Spacer(Modifier.width(8.dp))
                    Text(
                        account.name,
                        style = MaterialTheme.typography.labelLarge,
                        color = t.muted,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                Spacer(Modifier.height(10.dp))
                Text(
                    masked(account.balance),
                    style = MaterialTheme.typography.titleMedium.merge(Figures),
                    color = if (account.negative) t.red else t.text,
                    maxLines = 1,
                )
                val pending = account.pending.toInt()
                Text(
                    if (pending > 0) "$pending à pointer" else "À jour",
                    style = MaterialTheme.typography.bodySmall,
                    color = if (pending > 0) t.yellow else t.faint,
                )
            }
        }
    }
}

/** Recettes et dépenses du mois, en barre de proportion. */
@Composable
private fun MonthCard(dash: Dashboard) {
    val t = LocalTokyo.current
    TnCard {
        SectionTitle("${dash.month.replaceFirstChar { it.uppercase() }} en cours")
        Spacer(Modifier.height(14.dp))
        Row {
            Flow("Recettes", dash.income, t.green, Modifier.weight(1f))
            Flow("Dépenses", dash.expenses, t.red, Modifier.weight(1f))
        }
        Spacer(Modifier.height(14.dp))
        val total = dash.incomeValue + dash.expensesValue
        Row(
            Modifier
                .fillMaxWidth()
                .height(10.dp)
                .clip(RoundedCornerShape(50)),
        ) {
            if (total > 0) {
                if (dash.incomeValue > 0) Box(Modifier.weight(dash.incomeValue.toFloat()).fillMaxSize().background(t.green))
                if (dash.incomeValue > 0 && dash.expensesValue > 0) Spacer(Modifier.width(3.dp))
                if (dash.expensesValue > 0) Box(Modifier.weight(dash.expensesValue.toFloat()).fillMaxSize().background(t.red))
            } else {
                Box(Modifier.fillMaxSize().background(t.cardHigh))
            }
        }
        Spacer(Modifier.height(12.dp))
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text("Solde du mois", color = t.muted, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.weight(1f))
            Text(
                masked((if (!dash.netNegative) "+" else "") + dash.net),
                color = if (dash.netNegative) t.red else t.green,
                style = MaterialTheme.typography.titleMedium.merge(Figures),
            )
        }
        if (dash.expensesTrend.isNotEmpty()) {
            Spacer(Modifier.height(6.dp))
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(
                    if (dash.expensesUp) Icons.TrendUp else Icons.TrendDown,
                    contentDescription = null,
                    tint = if (dash.expensesUp) t.orange else t.teal,
                    modifier = Modifier.size(16.dp),
                )
                Spacer(Modifier.width(6.dp))
                Text(
                    "Dépenses : ${dash.expensesTrend}",
                    color = t.muted,
                    style = MaterialTheme.typography.bodySmall,
                )
            }
        }
        if (dash.pendingCount > 0u) {
            Spacer(Modifier.height(4.dp))
            Text(
                "${dash.pendingCount} opérations à pointer (${masked(dash.pending)})",
                color = t.faint,
                style = MaterialTheme.typography.bodySmall,
            )
        }
    }
}

@Composable
private fun Flow(label: String, value: String, color: Color, modifier: Modifier) {
    val t = LocalTokyo.current
    Column(modifier) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Box(
                Modifier
                    .size(8.dp)
                    .clip(CircleShape)
                    .background(color),
            )
            Spacer(Modifier.width(6.dp))
            Text(label, color = t.muted, style = MaterialTheme.typography.labelLarge)
        }
        Text(masked(value), style = MaterialTheme.typography.titleLarge.merge(Figures), color = t.text)
    }
}

/** Anneau des dépenses du mois par catégorie. */
@Composable
private fun CategoriesCard(dash: Dashboard, onMore: () -> Unit) {
    val t = LocalTokyo.current
    TnCard(Modifier.clickable(onClick = onMore)) {
        SectionTitle("Dépenses par catégorie") {
            Text("Analyse ›", color = t.blue, style = MaterialTheme.typography.labelLarge)
        }
        Spacer(Modifier.height(14.dp))
        Row(verticalAlignment = Alignment.CenterVertically) {
            Donut(
                shares = dash.categories.map { it.share },
                colors = t.chart,
                track = t.cardHigh,
                size = 132.dp,
                thickness = 16.dp,
            ) {
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    Text("Total", color = t.faint, style = MaterialTheme.typography.labelSmall)
                    Text(masked(dash.expenses), style = MaterialTheme.typography.labelLarge.merge(Figures), color = t.text)
                }
            }
            Spacer(Modifier.width(16.dp))
            Column(verticalArrangement = Arrangement.spacedBy(7.dp)) {
                dash.categories.forEachIndexed { i, c ->
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Box(
                            Modifier
                                .size(10.dp)
                                .clip(RoundedCornerShape(3.dp))
                                .background(t.chartColor(i)),
                        )
                        Spacer(Modifier.width(8.dp))
                        Text(
                            c.label,
                            style = MaterialTheme.typography.bodySmall,
                            color = t.muted,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.weight(1f),
                        )
                        Text(
                            "${(c.share * 100).toInt()} %",
                            style = MaterialTheme.typography.labelMedium.merge(Figures),
                            color = t.text,
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun UpcomingCard(items: List<UpcomingItem>, hasSchedules: Boolean, onSchedules: () -> Unit) {
    val t = LocalTokyo.current
    TnCard(if (hasSchedules) Modifier.clickable(onClick = onSchedules) else Modifier) {
        SectionTitle("À venir") {
            if (hasSchedules) Text("Échéancier ›", color = t.blue, style = MaterialTheme.typography.labelLarge)
        }
        Spacer(Modifier.height(8.dp))
        items.forEach { item ->
            Row(Modifier.padding(vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                DateBadge(item.date, item.days)
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(
                            item.payee.ifEmpty { "Échéance" },
                            style = MaterialTheme.typography.bodyLarge,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.weight(1f, fill = false),
                        )
                        if (item.scheduled) {
                            Spacer(Modifier.width(6.dp))
                            Icon(Icons.Repeat, contentDescription = "Échéance", tint = t.purple, modifier = Modifier.size(14.dp))
                        }
                    }
                    Text(item.account, style = MaterialTheme.typography.bodySmall, color = t.faint, maxLines = 1)
                }
                Text(
                    masked(item.amount),
                    style = MaterialTheme.typography.bodyLarge.merge(Figures),
                    fontWeight = FontWeight.SemiBold,
                    color = if (item.negative) t.text else t.green,
                )
            }
        }
    }
}

/** Date en pastille : jour en gros, délai dessous. */
@Composable
private fun DateBadge(date: String, days: Int) {
    val t = LocalTokyo.current
    val soon = days <= 3
    Column(
        Modifier
            .size(46.dp)
            .clip(RoundedCornerShape(14.dp))
            .background(if (soon) t.orange.copy(alpha = 0.16f) else t.cardHigh),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Text(
            date.substring(0, 2),
            style = MaterialTheme.typography.titleMedium.merge(Figures),
            color = if (soon) t.orange else t.text,
        )
        Text(
            when (days) {
                0 -> "auj."
                1 -> "demain"
                else -> "J-$days"
            },
            style = MaterialTheme.typography.labelSmall,
            color = if (soon) t.orange else t.faint,
        )
    }
}

@Composable
private fun RecentCard(items: List<RecentItem>, onOpen: (RecentItem) -> Unit) {
    val t = LocalTokyo.current
    TnCard(padding = 8.dp) {
        SectionTitle("Dernières opérations", Modifier.padding(start = 10.dp, top = 10.dp, end = 10.dp))
        Spacer(Modifier.height(4.dp))
        items.forEach { item ->
            Row(
                Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(16.dp))
                    .clickable { onOpen(item) }
                    .padding(10.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                // Initiale du tiers, couleur de la catégorie.
                Avatar(item.payee, colorFor(item.category.substringBefore(" > ").ifEmpty { item.payee }))
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f)) {
                    Text(item.payee, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(
                        listOf(item.date, item.category.ifEmpty { item.account }).joinToString(" · "),
                        style = MaterialTheme.typography.bodySmall,
                        color = t.faint,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                Text(
                    masked(item.amount),
                    style = MaterialTheme.typography.bodyLarge.merge(Figures),
                    fontWeight = FontWeight.SemiBold,
                    color = if (item.negative) t.text else t.green,
                )
            }
        }
    }
}
