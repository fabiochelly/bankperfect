package net.chelly.bankperfect.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.ExperimentalFoundationApi
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
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.draw.clip
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.layout.offset
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.IntOffset
import kotlin.math.roundToInt
import kotlinx.coroutines.launch
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import net.chelly.bankperfect.Model
import net.chelly.bankperfect.core.AccountView
import net.chelly.bankperfect.core.OperationRow
import net.chelly.bankperfect.core.TackState
import net.chelly.bankperfect.core.TackView

@OptIn(ExperimentalMaterial3Api::class, ExperimentalFoundationApi::class)
@Composable
fun Operations(model: Model, snackbar: SnackbarHostState) {
    BackHandler { model.closeAccount() }
    val view = model.view
    val list = rememberLazyListState()
    Scaffold(
        topBar = {
            TopAppBar(
                navigationIcon = {
                    IconButton(onClick = model::closeAccount) { Icon(Icons.Back, contentDescription = "Comptes") }
                },
                title = { Text(view?.name ?: "", maxLines = 1, overflow = TextOverflow.Ellipsis) },
                actions = {
                    SavingIndicator(model.saving)
                    HideButton(model)
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = MaterialTheme.colorScheme.background),
            )
        },
        floatingActionButton = {
            FloatingActionButton(
                onClick = model::startNew,
                containerColor = MaterialTheme.colorScheme.primary,
                contentColor = MaterialTheme.colorScheme.onPrimary,
            ) { Icon(Icons.Add, contentDescription = "Nouvelle opération") }
        },
        snackbarHost = { SnackbarHost(snackbar) },
    ) { padding ->
        Column(
            Modifier
                .padding(top = padding.calculateTopPadding())
                .fillMaxSize(),
        ) {
            if (view != null) AccountHeader(view)
            OutlinedTextField(
                value = model.search,
                onValueChange = model::changeSearch,
                placeholder = { Text("Rechercher", maxLines = 1) },
                leadingIcon = { Icon(Icons.Search, contentDescription = null) },
                trailingIcon = {
                    if (model.search.isNotEmpty()) {
                        IconButton(onClick = { model.changeSearch("") }) {
                            Icon(Icons.Close, contentDescription = "Effacer")
                        }
                    }
                },
                singleLine = true,
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp),
            )
            Row(
                Modifier.padding(horizontal = 16.dp, vertical = 4.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                for ((value, label) in listOf(
                    TackView.ALL to "Toutes",
                    TackView.PENDING to "À pointer",
                    TackView.TACKED to "Pointées",
                )) {
                    FilterChip(
                        selected = model.tackView == value,
                        onClick = { model.changeTackView(value) },
                        label = { Text(label) },
                    )
                }
            }
            if (view != null && view.rows.isEmpty()) {
                Box(
                    Modifier
                        .fillMaxSize()
                        .padding(32.dp),
                    contentAlignment = Alignment.TopCenter,
                ) {
                    Text(
                        if (view.total == 0u) "Aucune opération" else "Aucune opération ne correspond",
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
            val rows = view?.rows ?: emptyList()
            LazyColumn(
                state = list,
                contentPadding = PaddingValues(bottom = padding.calculateBottomPadding() + 88.dp),
            ) {
                var month: String? = null
                for ((i, row) in rows.withIndex()) {
                    if (row.month != month) {
                        month = row.month
                        stickyHeader(key = "m$i") { MonthHeader(row.month) }
                    }
                    item(key = row.uid.toLong()) {
                        OperationLine(
                            row,
                            onTack = { model.toggleTack(row.uid) },
                            onOpen = { model.startEdit(row.uid) },
                        )
                    }
                }
            }
        }
    }
}

/** Solde du compte sur le dégradé de l'accueil. */
@Composable
private fun AccountHeader(view: AccountView) {
    val t = LocalTokyo.current
    val on = t.onHero
    Row(
        Modifier
            .padding(horizontal = 16.dp, vertical = 4.dp)
            .fillMaxWidth()
            .clip(RoundedCornerShape(24.dp))
            .background(t.hero)
            .padding(horizontal = 20.dp, vertical = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text("Solde", color = on.copy(alpha = 0.7f), style = MaterialTheme.typography.labelLarge)
            Text(
                masked(view.balance),
                color = on,
                style = MaterialTheme.typography.headlineMedium.merge(Figures),
                fontWeight = FontWeight.Bold,
            )
        }
        Column(horizontalAlignment = Alignment.End) {
            Text("En banque", color = on.copy(alpha = 0.7f), style = MaterialTheme.typography.labelMedium)
            Text(masked(view.pointed), color = on, style = MaterialTheme.typography.titleSmall.merge(Figures))
        }
    }
    Spacer(Modifier.height(8.dp))
}

@Composable
private fun MonthHeader(month: String) {
    Text(
        month.replaceFirstChar { it.uppercase() },
        style = MaterialTheme.typography.labelLarge,
        color = MaterialTheme.colorScheme.primary,
        modifier = Modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.background)
            .padding(horizontal = 16.dp, vertical = 6.dp),
    )
}

/**
 * Ligne d'opération. Glisser vers la droite pointe ou dépointe, comme la
 * pastille ; le fond annonce ce que fera le geste.
 */
@Composable
private fun OperationLine(row: OperationRow, onTack: () -> Unit, onOpen: () -> Unit) {
    val money = LocalMoney.current
    val t = LocalTokyo.current
    val scope = rememberCoroutineScope()
    val offset = remember { Animatable(0f) }
    val threshold = with(LocalDensity.current) { 96.dp.toPx() }
    val armed = offset.value > threshold
    Column(Modifier.alpha(if (row.future) 0.6f else 1f)) {
      Box {
        if (offset.value > 0f) {
            Row(
                Modifier
                    .matchParentSize()
                    .background(if (armed) t.blue.copy(alpha = 0.28f) else t.blue.copy(alpha = 0.12f))
                    .padding(start = 20.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Icon(Icons.Check, contentDescription = null, tint = t.blue)
                Spacer(Modifier.width(8.dp))
                Text(
                    if (row.tack == TackState.NONE) "Pointer" else "Dépointer",
                    color = t.blue,
                    style = MaterialTheme.typography.labelLarge,
                )
            }
        }
        Row(
            Modifier
                .offset { IntOffset(offset.value.roundToInt(), 0) }
                .fillMaxWidth()
                .background(MaterialTheme.colorScheme.background)
                .pointerInput(row.uid) {
                    detectHorizontalDragGestures(
                        onDragEnd = {
                            val done = offset.value > threshold
                            scope.launch {
                                if (done) onTack()
                                offset.animateTo(0f)
                            }
                        },
                        onDragCancel = { scope.launch { offset.animateTo(0f) } },
                    ) { change, amount ->
                        change.consume()
                        scope.launch { offset.snapTo((offset.value + amount).coerceIn(0f, threshold * 1.6f)) }
                    }
                }
                .clickable(onClick = onOpen)
                .padding(start = 8.dp, end = 16.dp, top = 8.dp, bottom = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            TackButton(row.tack, onTack)
            Spacer(Modifier.width(4.dp))
            Column(Modifier.weight(1f)) {
                Text(
                    row.payee.ifEmpty { row.mode },
                    style = MaterialTheme.typography.bodyLarge,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                val sub = listOf(row.date.substring(0, 5), row.category, row.details)
                    .filter { it.isNotEmpty() }
                    .joinToString(" · ")
                Text(
                    sub,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            Spacer(Modifier.width(12.dp))
            Column(horizontalAlignment = Alignment.End) {
                Text(
                    masked(row.amount),
                    style = MaterialTheme.typography.bodyLarge.merge(Figures),
                    fontWeight = FontWeight.SemiBold,
                    color = if (row.negative) MaterialTheme.colorScheme.onSurface else money.positive,
                )
                Text(
                    masked(row.balance),
                    style = MaterialTheme.typography.bodySmall.merge(Figures),
                    color = if (row.balanceNegative) money.negative else MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
      }
        HorizontalDivider(
            Modifier.padding(start = 56.dp),
            color = MaterialTheme.colorScheme.outline.copy(alpha = 0.35f),
        )
    }
}

/** Pastille de pointage : vide, cochée (pointée) ou pleine (rapprochée). Zone d'appui de 48 dp. */
@Composable
private fun TackButton(tack: TackState, onClick: () -> Unit) {
    val primary = MaterialTheme.colorScheme.primary
    Box(
        Modifier
            .size(48.dp)
            .clickable(onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        when (tack) {
            TackState.NONE -> Box(
                Modifier
                    .size(22.dp)
                    .border(2.dp, MaterialTheme.colorScheme.outline, CircleShape),
            )
            TackState.POINTED -> Box(
                Modifier
                    .size(22.dp)
                    .background(primary, CircleShape),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    Icons.Check,
                    contentDescription = "Pointée",
                    tint = MaterialTheme.colorScheme.onPrimary,
                    modifier = Modifier.size(16.dp),
                )
            }
            TackState.RECONCILED -> Box(
                Modifier
                    .size(22.dp)
                    .background(MaterialTheme.colorScheme.secondary, CircleShape),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    "R",
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.background,
                    fontWeight = FontWeight.Bold,
                )
            }
        }
    }
    Spacer(Modifier.height(0.dp))
}
