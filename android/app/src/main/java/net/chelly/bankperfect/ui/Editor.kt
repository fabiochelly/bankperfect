package net.chelly.bankperfect.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.AssistChip
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
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
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneOffset
import java.time.format.DateTimeFormatter
import kotlinx.coroutines.launch
import net.chelly.bankperfect.Editing
import net.chelly.bankperfect.Model
import net.chelly.bankperfect.core.CategoryGuess
import net.chelly.bankperfect.core.CategoryItem
import net.chelly.bankperfect.core.Draft
import net.chelly.bankperfect.core.PayeeChoice
import net.chelly.bankperfect.core.TackState

private val FRENCH_DATE = DateTimeFormatter.ofPattern("dd/MM/yyyy")

private fun parseDate(text: String): LocalDate? = runCatching { LocalDate.parse(text, FRENCH_DATE) }.getOrNull()

/** Formulaire de saisie ou de modification d'une opération, plein écran. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun Editor(model: Model, editing: Editing) {
    BackHandler { model.cancelEdit() }
    val scope = rememberCoroutineScope()
    val modes = remember { model.modes() }
    val categories = remember { model.categories() }
    var draft by remember(editing) { mutableStateOf(editing.draft) }
    // Le montant garde sa position de curseur entre deux recompositions.
    var amount by remember(editing) {
        mutableStateOf(TextFieldValue(editing.draft.amount, TextRange(editing.draft.amount.length)))
    }
    var error by remember { mutableStateOf<String?>(null) }
    var choices by remember { mutableStateOf<List<PayeeChoice>>(emptyList()) }
    var payeeFocused by remember { mutableStateOf(false) }
    var guess by remember { mutableStateOf<CategoryGuess?>(null) }
    var warning by remember { mutableStateOf<String?>(null) }
    // Catégorie choisie à la main : la proposition automatique ne la remplace pas.
    var categoryTouched by remember(editing) { mutableStateOf(editing.draft.category != null) }
    var pickCategory by remember { mutableStateOf(false) }
    var pickDate by remember { mutableStateOf(false) }
    var confirmDelete by remember { mutableStateOf(false) }
    val amountFocus = remember { FocusRequester() }
    val isNew = editing.uid == null

    fun current() = draft.copy(amount = amount.text)

    // Nouvelle opération : clavier numérique d'emblée sur le montant.
    LaunchedEffect(editing) { if (isNew) amountFocus.requestFocus() }
    LaunchedEffect(draft.payee, payeeFocused) {
        choices = if (payeeFocused) model.payeeChoices(editing.account, draft.payee) else emptyList()
    }
    LaunchedEffect(draft.payee, draft.details, draft.credit, draft.mode) {
        val proposed = model.guessCategory(current())
        guess = proposed
        if (!categoryTouched && proposed != null && proposed.confident) {
            draft = draft.copy(category = proposed.id)
        }
    }
    LaunchedEffect(amount.text, draft.credit, draft.mode) { warning = model.modeWarning(current()) }

    fun submit() {
        scope.launch { error = model.commit(current()) }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                navigationIcon = {
                    IconButton(onClick = model::cancelEdit) { Icon(Icons.Close, contentDescription = "Annuler") }
                },
                title = { Text(if (isNew) "Nouvelle opération" else "Opération") },
                actions = {
                    TextButton(onClick = ::submit) { Text("Enregistrer", fontWeight = FontWeight.SemiBold) }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = MaterialTheme.colorScheme.background),
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
            SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth()) {
                SegmentedButton(
                    selected = !draft.credit,
                    onClick = { draft = draft.copy(credit = false) },
                    shape = SegmentedButtonDefaults.itemShape(0, 2),
                ) { Text("Dépense") }
                SegmentedButton(
                    selected = draft.credit,
                    onClick = { draft = draft.copy(credit = true) },
                    shape = SegmentedButtonDefaults.itemShape(1, 2),
                ) { Text("Recette") }
            }
            val money = LocalMoney.current
            OutlinedTextField(
                value = amount,
                onValueChange = { amount = it.copy(text = it.text.filter { c -> c.isDigit() || c == ',' || c == '.' }) },
                label = { Text("Montant") },
                prefix = { Text(if (draft.credit) "+ " else "− ") },
                suffix = { Text("€") },
                singleLine = true,
                textStyle = MaterialTheme.typography.headlineSmall.copy(
                    fontWeight = FontWeight.SemiBold,
                    color = if (draft.credit) money.positive else MaterialTheme.colorScheme.onSurface,
                ),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal, imeAction = ImeAction.Next),
                modifier = Modifier
                    .fillMaxWidth()
                    .focusRequester(amountFocus),
            )
            warning?.let {
                Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error)
            }

            Column {
                OutlinedTextField(
                    value = draft.payee,
                    onValueChange = { draft = draft.copy(payee = it) },
                    label = { Text("Tiers") },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(
                        capitalization = KeyboardCapitalization.Sentences,
                        imeAction = ImeAction.Next,
                    ),
                    modifier = Modifier
                        .fillMaxWidth()
                        .onFocusChanged { payeeFocused = it.isFocused },
                )
                if (choices.isNotEmpty()) {
                    Card(
                        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainerHigh),
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(top = 4.dp),
                    ) {
                        for (choice in choices) {
                            Text(
                                (if (choice.memorized) "★ " else "") + choice.caption,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .clickable {
                                        scope.launch {
                                            val filled = model.fillFromPayee(editing.account, current(), choice)
                                            if (filled.category != draft.category) categoryTouched = true
                                            draft = filled
                                            if (filled.amount != amount.text) {
                                                amount = TextFieldValue(filled.amount, TextRange(filled.amount.length))
                                            }
                                            choices = emptyList()
                                        }
                                    }
                                    .padding(horizontal = 16.dp, vertical = 12.dp),
                            )
                        }
                    }
                }
            }

            val category = categories.firstOrNull { it.id == draft.category }
            PickerField("Catégorie", category?.label ?: "Aucune") { pickCategory = true }
            val proposal = guess
            if (proposal != null && proposal.id != draft.category) {
                AssistChip(
                    onClick = {
                        draft = draft.copy(category = proposal.id)
                        categoryTouched = true
                    },
                    label = { Text("Proposée : ${proposal.label}", maxLines = 1, overflow = TextOverflow.Ellipsis) },
                )
            }

            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Box(Modifier.weight(1f)) {
                    ModeField(modes.map { it.label }, draft.mode.toInt()) { index ->
                        val mode = modes[index]
                        draft = draft.copy(mode = mode.index, credit = !mode.debit)
                    }
                }
                Box(Modifier.weight(1f)) {
                    PickerField("Date", draft.date) { pickDate = true }
                }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                val today = LocalDate.now()
                AssistChip(onClick = { draft = draft.copy(date = today.format(FRENCH_DATE)) }, label = { Text("Aujourd'hui") })
                AssistChip(
                    onClick = { draft = draft.copy(date = today.minusDays(1).format(FRENCH_DATE)) },
                    label = { Text("Hier") },
                )
            }

            OutlinedTextField(
                value = draft.details,
                onValueChange = { draft = draft.copy(details = it) },
                label = { Text("Détail") },
                singleLine = true,
                keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences, imeAction = ImeAction.Done),
                modifier = Modifier.fillMaxWidth(),
            )

            Text("Pointage", style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
            SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth()) {
                val states = listOf(TackState.NONE to "Non pointée", TackState.POINTED to "Pointée", TackState.RECONCILED to "Rapprochée")
                states.forEachIndexed { i, (state, label) ->
                    SegmentedButton(
                        selected = draft.tack == state,
                        onClick = { draft = draft.copy(tack = state) },
                        shape = SegmentedButtonDefaults.itemShape(i, states.size),
                    ) { Text(label, maxLines = 1) }
                }
            }

            error?.let { Text(it, color = MaterialTheme.colorScheme.error) }

            if (!isNew) {
                Spacer(Modifier.width(0.dp))
                OutlinedButton(onClick = { confirmDelete = true }, modifier = Modifier.fillMaxWidth()) {
                    Icon(Icons.Delete, contentDescription = null)
                    Spacer(Modifier.width(8.dp))
                    Text("Supprimer l'opération")
                }
            }
        }
    }

    if (pickCategory) {
        CategoryPicker(
            categories,
            onPick = {
                draft = draft.copy(category = it)
                categoryTouched = true
                pickCategory = false
            },
            onDismiss = { pickCategory = false },
        )
    }
    if (pickDate) {
        val initial = parseDate(draft.date) ?: LocalDate.now()
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
    if (confirmDelete) {
        AlertDialog(
            onDismissRequest = { confirmDelete = false },
            title = { Text("Supprimer cette opération ?") },
            confirmButton = {
                TextButton(onClick = {
                    confirmDelete = false
                    editing.uid?.let(model::delete)
                }) { Text("Supprimer", color = MaterialTheme.colorScheme.error) }
            },
            dismissButton = { TextButton(onClick = { confirmDelete = false }) { Text("Annuler") } },
        )
    }
}

/** Champ en lecture seule qui ouvre un choix. */
@Composable
private fun PickerField(label: String, value: String, onClick: () -> Unit) {
    Box {
        OutlinedTextField(
            value = value,
            onValueChange = {},
            label = { Text(label) },
            singleLine = true,
            readOnly = true,
            enabled = false,
            colors = OutlinedTextFieldDefaults.colors(
                disabledTextColor = MaterialTheme.colorScheme.onSurface,
                disabledBorderColor = MaterialTheme.colorScheme.outline,
                disabledLabelColor = MaterialTheme.colorScheme.onSurfaceVariant,
            ),
            modifier = Modifier.fillMaxWidth(),
        )
        Box(
            Modifier
                .matchParentSize()
                .clickable(onClick = onClick),
        )
    }
}

@Composable
private fun ModeField(labels: List<String>, selected: Int, onSelect: (Int) -> Unit) {
    var open by remember { mutableStateOf(false) }
    Box {
        PickerField("Mode", labels.getOrElse(selected) { "" }) { open = true }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            labels.forEachIndexed { i, label ->
                DropdownMenuItem(text = { Text(label) }, onClick = {
                    open = false
                    onSelect(i)
                })
            }
        }
    }
}

/** Choix d'une catégorie, avec recherche. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun CategoryPicker(categories: List<CategoryItem>, onPick: (UInt?) -> Unit, onDismiss: () -> Unit) {
    var filter by remember { mutableStateOf("") }
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { focus.requestFocus() }
    AlertDialog(
        onDismissRequest = onDismiss,
        confirmButton = { TextButton(onClick = onDismiss) { Text("Annuler") } },
        title = { Text("Catégorie") },
        text = {
            Column {
                OutlinedTextField(
                    value = filter,
                    onValueChange = { filter = it },
                    placeholder = { Text("Rechercher") },
                    leadingIcon = { Icon(Icons.Search, contentDescription = null) },
                    singleLine = true,
                    modifier = Modifier
                        .fillMaxWidth()
                        .focusRequester(focus),
                )
                val needle = filter.trim().lowercase()
                val shown = if (needle.isEmpty()) categories else categories.filter { it.label.lowercase().contains(needle) }
                LazyColumn(Modifier.fillMaxSize()) {
                    if (needle.isEmpty()) {
                        item {
                            Text(
                                "Aucune",
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .clickable { onPick(null) }
                                    .padding(vertical = 12.dp),
                            )
                        }
                    }
                    items(shown, key = { it.id.toLong() }) { category ->
                        Text(
                            if (needle.isEmpty()) category.name else category.label,
                            fontSize = 16.sp,
                            fontWeight = if (category.child || needle.isNotEmpty()) FontWeight.Normal else FontWeight.SemiBold,
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable { onPick(category.id) }
                                .padding(start = if (category.child && needle.isEmpty()) 20.dp else 0.dp, top = 12.dp, bottom = 12.dp),
                        )
                        HorizontalDivider(color = MaterialTheme.colorScheme.outline.copy(alpha = 0.25f))
                    }
                }
            }
        },
    )
}
