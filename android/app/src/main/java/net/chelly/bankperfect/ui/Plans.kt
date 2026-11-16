package net.chelly.bankperfect.ui

import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.AssistChip
import androidx.compose.material3.Button
import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.rememberDatePickerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneOffset
import java.time.format.DateTimeFormatter
import kotlinx.coroutines.launch
import net.chelly.bankperfect.Model
import net.chelly.bankperfect.Page
import net.chelly.bankperfect.core.AccountSummary
import net.chelly.bankperfect.core.BudgetLine
import net.chelly.bankperfect.core.BudgetStatus
import net.chelly.bankperfect.core.ScheduleItem

private val FRENCH_DATE = DateTimeFormatter.ofPattern("dd/MM/yyyy")

// ── Budget ───────────────────────────────────────────────────────────────

/**
 * Budget du mois : jauge globale puis une par catégorie. Le trait vertical
 * marque la part du mois écoulée : une jauge qui le dépasse va trop vite.
 */
@Composable
fun BudgetCard(budget: BudgetStatus, title: String = "Budget du mois", limit: Int = 4) {
    val t = LocalTokyo.current
    var all by remember { mutableStateOf(false) }
    TnCard {
        SectionTitle(title) {
            Text(
                "${(budget.ratio * 100).toInt()} %",
                style = MaterialTheme.typography.labelLarge.merge(Figures),
                color = if (budget.over) t.red else t.muted,
            )
        }
        Spacer(Modifier.height(10.dp))
        Row(verticalAlignment = Alignment.Bottom) {
            Text(
                masked(budget.spent),
                style = MaterialTheme.typography.titleLarge.merge(Figures),
                color = t.text,
            )
            Text(
                " sur ${masked(budget.planned)}",
                style = MaterialTheme.typography.bodyMedium.merge(Figures),
                color = t.faint,
                modifier = Modifier.padding(bottom = 2.dp),
            )
        }
        Spacer(Modifier.height(8.dp))
        Gauge(budget.ratio, budget.elapsed, gaugeColor(budget.ratio, budget.elapsed, budget.over), Modifier.height(10.dp))
        Spacer(Modifier.height(6.dp))
        Text(
            if (budget.over) "Dépassé de ${masked(budget.left)}" else "Reste ${masked(budget.left)}",
            style = MaterialTheme.typography.bodySmall,
            color = if (budget.over) t.red else t.muted,
        )
        val shown = if (all) budget.lines else budget.lines.take(limit)
        shown.forEach { line ->
            Spacer(Modifier.height(14.dp))
            BudgetRow(line, budget.elapsed)
        }
        if (budget.lines.size > limit) {
            Spacer(Modifier.height(8.dp))
            Text(
                if (all) "Moins de détails" else "Toutes les catégories (${budget.lines.size})",
                color = t.blue,
                style = MaterialTheme.typography.labelLarge,
                modifier = Modifier
                    .clip(RoundedCornerShape(8.dp))
                    .clickable { all = !all }
                    .padding(vertical = 6.dp),
            )
        }
    }
}

@Composable
private fun BudgetRow(line: BudgetLine, elapsed: Double) {
    val t = LocalTokyo.current
    Column {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                line.label,
                style = MaterialTheme.typography.bodyMedium,
                fontWeight = FontWeight.SemiBold,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f),
            )
            Text(
                "${masked(line.spent)} / ${masked(line.planned)}",
                style = MaterialTheme.typography.bodySmall.merge(Figures),
                color = if (line.over) t.red else t.muted,
            )
        }
        Spacer(Modifier.height(6.dp))
        Gauge(line.ratio, elapsed, gaugeColor(line.ratio, elapsed, line.over), Modifier.height(6.dp))
    }
}

/** Vert tant que la dépense suit le mois, orange en avance, rouge au-delà. */
@Composable
private fun gaugeColor(ratio: Double, elapsed: Double, over: Boolean): Color {
    val t = LocalTokyo.current
    return when {
        over -> t.red
        ratio > elapsed + 0.1 -> t.orange
        else -> t.teal
    }
}

@Composable
private fun Gauge(ratio: Double, elapsed: Double, color: Color, modifier: Modifier) {
    val t = LocalTokyo.current
    val target = ratio.coerceIn(0.0, 1.0).toFloat()
    val fill by animateFloatAsState(target, tween(700), label = "jauge")
    BoxWithConstraints(
        modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(50))
            .background(t.cardHigh),
    ) {
        Box(
            Modifier
                .fillMaxHeight()
                .fillMaxWidth(fill)
                .clip(RoundedCornerShape(50))
                .background(color),
        )
        if (elapsed in 0.01..0.99) {
            Box(
                Modifier
                    .offset(x = maxWidth * elapsed.toFloat() - 1.dp)
                    .width(2.dp)
                    .fillMaxHeight()
                    .background(t.text.copy(alpha = 0.55f)),
            )
        }
    }
}

// ── Échéancier ───────────────────────────────────────────────────────────

/** Échéances à venir ; chacune peut être saisie d'un geste quand elle tombe. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun Schedules(model: Model, snackbar: SnackbarHostState) {
    BackHandler { model.page = null }
    val t = LocalTokyo.current
    var confirm by remember { mutableStateOf<ScheduleItem?>(null) }
    Scaffold(
        containerColor = t.page,
        topBar = {
            TopAppBar(
                navigationIcon = {
                    IconButton(onClick = { model.page = null }) { Icon(Icons.Back, contentDescription = "Retour") }
                },
                title = { Text("Échéancier") },
                actions = { SavingIndicator(model.saving) },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = t.page),
            )
        },
        snackbarHost = { SnackbarHost(snackbar) },
    ) { padding ->
        val items = model.schedules
        LazyColumn(
            contentPadding = PaddingValues(
                start = 16.dp,
                end = 16.dp,
                top = padding.calculateTopPadding() + 4.dp,
                bottom = padding.calculateBottomPadding() + 24.dp,
            ),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            if (items.isEmpty()) {
                item {
                    Text(
                        "Aucune échéance. Elles se créent dans BankPerfect sur ordinateur (menu Comptes › Échéancier, Ctrl+E).",
                        color = t.faint,
                        modifier = Modifier.padding(16.dp),
                    )
                }
            } else {
                item {
                    Text(
                        "Les échéances automatiques sont saisies par BankPerfect sur ordinateur à l'ouverture du fichier. " +
                            "Ici, « Saisir » insère la prochaine occurrence tout de suite.",
                        color = t.faint,
                        style = MaterialTheme.typography.bodySmall,
                        modifier = Modifier.padding(horizontal = 4.dp, vertical = 4.dp),
                    )
                }
            }
            items(items, key = { it.index.toInt() }) { item ->
                ScheduleCard(item) { confirm = item }
            }
        }
    }
    confirm?.let { item ->
        AlertDialog(
            onDismissRequest = { confirm = null },
            title = { Text("Saisir cette échéance ?") },
            text = {
                Text(
                    "« ${item.payee} » du ${item.date}, ${item.amount}, dans ${item.account}" +
                        (if (item.transferTo.isNotEmpty()) " vers ${item.transferTo}" else "") +
                        ". L'échéance passera à la date suivante.",
                )
            },
            confirmButton = {
                TextButton(onClick = {
                    confirm = null
                    model.insertSchedule(item)
                }) { Text("Saisir") }
            },
            dismissButton = { TextButton(onClick = { confirm = null }) { Text("Annuler") } },
        )
    }
}

@Composable
private fun ScheduleCard(item: ScheduleItem, onInsert: () -> Unit) {
    val t = LocalTokyo.current
    val late = item.days < 0
    val soon = item.days in 0..3
    TnCard(padding = 14.dp) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(
                Modifier
                    .size(52.dp)
                    .clip(RoundedCornerShape(16.dp))
                    .background(
                        when {
                            late -> t.red.copy(alpha = 0.16f)
                            soon -> t.orange.copy(alpha = 0.16f)
                            else -> t.cardHigh
                        },
                    ),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.Center,
            ) {
                val tint = when {
                    late -> t.red
                    soon -> t.orange
                    else -> t.text
                }
                Text(item.date.substring(0, 5), style = MaterialTheme.typography.labelLarge.merge(Figures), color = tint)
                Text(
                    when {
                        late -> "retard"
                        item.days == 0 -> "auj."
                        item.days == 1 -> "demain"
                        else -> "J-${item.days}"
                    },
                    style = MaterialTheme.typography.labelSmall,
                    color = if (late || soon) tint else t.faint,
                )
            }
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f)) {
                Text(
                    item.payee.ifEmpty { "Échéance" },
                    style = MaterialTheme.typography.bodyLarge,
                    fontWeight = FontWeight.SemiBold,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    item.account + (if (item.transferTo.isNotEmpty()) " → ${item.transferTo}" else ""),
                    style = MaterialTheme.typography.bodySmall,
                    color = t.muted,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Icon(Icons.Repeat, contentDescription = null, tint = t.purple, modifier = Modifier.size(13.dp))
                    Spacer(Modifier.width(4.dp))
                    Text(
                        item.interval + if (item.automatic) " · auto" else "",
                        style = MaterialTheme.typography.bodySmall,
                        color = t.faint,
                    )
                }
            }
            Spacer(Modifier.width(8.dp))
            Column(horizontalAlignment = Alignment.End) {
                Text(
                    masked(item.amount),
                    style = MaterialTheme.typography.bodyLarge.merge(Figures),
                    fontWeight = FontWeight.SemiBold,
                    color = if (item.negative) t.text else t.green,
                )
                Spacer(Modifier.height(4.dp))
                FilledTonalButton(
                    onClick = onInsert,
                    contentPadding = PaddingValues(horizontal = 14.dp, vertical = 0.dp),
                    modifier = Modifier.height(32.dp),
                ) { Text("Saisir", style = MaterialTheme.typography.labelLarge) }
            }
        }
    }
}

// ── Virement ─────────────────────────────────────────────────────────────

/** Virement d'un compte à l'autre : deux lignes liées, comme sur ordinateur. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TransferScreen(model: Model) {
    BackHandler { model.page = null }
    val t = LocalTokyo.current
    val scope = rememberCoroutineScope()
    var draft by remember { mutableStateOf(model.newTransfer()) }
    var error by remember { mutableStateOf<String?>(null) }
    var pickDate by remember { mutableStateOf(false) }
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { focus.requestFocus() }
    val accounts = model.accounts.filter { !it.disabled }

    fun submit() {
        scope.launch { error = model.commitTransfer(draft) }
    }

    Scaffold(
        containerColor = t.page,
        topBar = {
            TopAppBar(
                navigationIcon = {
                    IconButton(onClick = { model.page = null }) { Icon(Icons.Close, contentDescription = "Annuler") }
                },
                title = { Text("Virement") },
                actions = {
                    TextButton(onClick = ::submit) { Text("Enregistrer", fontWeight = FontWeight.SemiBold) }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = t.page),
            )
        },
    ) { padding ->
        Column(
            Modifier
                .padding(padding)
                .imePadding()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            OutlinedTextField(
                value = draft.amount,
                onValueChange = { text -> draft = draft.copy(amount = text.filter { it.isDigit() || it == ',' || it == '.' }) },
                label = { Text("Montant") },
                suffix = { Text("€") },
                singleLine = true,
                textStyle = MaterialTheme.typography.headlineSmall.copy(fontWeight = FontWeight.SemiBold),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal, imeAction = ImeAction.Next),
                modifier = Modifier
                    .fillMaxWidth()
                    .focusRequester(focus),
            )
            AccountChoice("Depuis", accounts, draft.from) { draft = draft.copy(from = it) }
            Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
                IconButton(onClick = { draft = draft.copy(from = draft.to, to = draft.from) }) {
                    Icon(Icons.Swap, contentDescription = "Inverser", tint = t.blue)
                }
            }
            AccountChoice("Vers", accounts, draft.to) { draft = draft.copy(to = it) }
            Box(Modifier.clickable { pickDate = true }) {
                OutlinedTextField(
                    value = draft.date,
                    onValueChange = {},
                    label = { Text("Date") },
                    readOnly = true,
                    enabled = false,
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                    colors = androidx.compose.material3.OutlinedTextFieldDefaults.colors(
                        disabledTextColor = MaterialTheme.colorScheme.onSurface,
                        disabledBorderColor = MaterialTheme.colorScheme.outline,
                        disabledLabelColor = MaterialTheme.colorScheme.onSurfaceVariant,
                    ),
                )
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                val today = LocalDate.now()
                AssistChip(onClick = { draft = draft.copy(date = today.format(FRENCH_DATE)) }, label = { Text("Aujourd'hui") })
                AssistChip(onClick = { draft = draft.copy(date = today.minusDays(1).format(FRENCH_DATE)) }, label = { Text("Hier") })
            }
            OutlinedTextField(
                value = draft.details,
                onValueChange = { draft = draft.copy(details = it) },
                label = { Text("Détail") },
                singleLine = true,
                keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences, imeAction = ImeAction.Done),
                modifier = Modifier.fillMaxWidth(),
            )
            error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
            Button(onClick = ::submit, modifier = Modifier.fillMaxWidth()) { Text("Virer") }
        }
    }
    if (pickDate) {
        val initial = runCatching { LocalDate.parse(draft.date, FRENCH_DATE) }.getOrNull() ?: LocalDate.now()
        val state = rememberDatePickerState(
            initialSelectedDateMillis = initial.atStartOfDay().toInstant(ZoneOffset.UTC).toEpochMilli(),
        )
        DatePickerDialog(
            onDismissRequest = { pickDate = false },
            confirmButton = {
                TextButton(onClick = {
                    state.selectedDateMillis?.let { millis ->
                        val date = Instant.ofEpochMilli(millis).atZone(ZoneOffset.UTC).toLocalDate()
                        draft = draft.copy(date = date.format(FRENCH_DATE))
                    }
                    pickDate = false
                }) { Text("OK") }
            },
            dismissButton = { TextButton(onClick = { pickDate = false }) { Text("Annuler") } },
        ) { DatePicker(state) }
    }
}

/** Compte choisi dans une liste, en carte avec son solde. */
@Composable
private fun AccountChoice(label: String, accounts: List<AccountSummary>, selected: UInt, onPick: (UInt) -> Unit) {
    val t = LocalTokyo.current
    var open by remember { mutableStateOf(false) }
    val account = accounts.firstOrNull { it.index == selected }
    Box {
        TnCard(Modifier.clickable { open = true }, padding = 14.dp) {
            Text(label, style = MaterialTheme.typography.labelMedium, color = t.faint)
            Spacer(Modifier.height(4.dp))
            Row(verticalAlignment = Alignment.CenterVertically) {
                val name = account?.name ?: "Choisir un compte"
                Avatar(name, colorFor(name), size = 36.dp)
                Spacer(Modifier.width(12.dp))
                Text(
                    name,
                    style = MaterialTheme.typography.titleMedium,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f),
                )
                account?.let {
                    Text(
                        masked(it.balance),
                        style = MaterialTheme.typography.bodyMedium.merge(Figures),
                        color = if (it.negative) t.red else t.muted,
                    )
                }
                Icon(Icons.ChevronDown, contentDescription = null, tint = t.faint)
            }
        }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            accounts.forEach { a ->
                DropdownMenuItem(
                    text = { Text(a.name) },
                    trailingIcon = { Text(masked(a.balance), style = MaterialTheme.typography.bodySmall.merge(Figures)) },
                    onClick = {
                        open = false
                        onPick(a.index)
                    },
                )
            }
        }
    }
}

/** Raccourci de l'accueil vers l'échéancier. */
@Composable
fun ScheduleLink(model: Model) {
    val t = LocalTokyo.current
    val count = model.schedules.size
    val late = model.schedules.count { it.days < 0 }
    TnCard(Modifier.clickable { model.page = Page.SCHEDULES }, padding = 16.dp) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Box(
                Modifier
                    .size(40.dp)
                    .clip(RoundedCornerShape(14.dp))
                    .background(t.purple.copy(alpha = 0.16f)),
                contentAlignment = Alignment.Center,
            ) { Icon(Icons.Calendar, contentDescription = null, tint = t.purple) }
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f)) {
                Text("Échéancier", style = MaterialTheme.typography.titleMedium)
                Text(
                    if (late > 0) "$late en retard · $count au total" else "$count échéances",
                    style = MaterialTheme.typography.bodySmall,
                    color = if (late > 0) t.red else t.faint,
                )
            }
            Icon(Icons.ChevronRight, contentDescription = null, tint = t.faint)
        }
    }
}
