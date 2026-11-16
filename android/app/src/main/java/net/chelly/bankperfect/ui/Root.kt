package net.chelly.bankperfect.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import net.chelly.bankperfect.Model
import net.chelly.bankperfect.Screen
import net.chelly.bankperfect.core.AccountSummary

@Composable
fun Root(model: Model, onPick: () -> Unit) {
    val snackbar = remember { SnackbarHostState() }
    LaunchedEffect(model.notice) {
        val notice = model.notice ?: return@LaunchedEffect
        snackbar.showSnackbar(notice, withDismissAction = true, duration = SnackbarDuration.Long)
        model.notice = null
    }
    when (val screen = model.screen) {
        Screen.Welcome -> Welcome(onPick)
        is Screen.Loading -> Loading(screen.message)
        is Screen.Password -> Password(
            screen,
            onUnlock = model::unlock,
            onBiometric = model::unlockWithBiometric,
            onOther = onPick,
        )
        is Screen.Failure -> Failure(screen.message, onRetry = model::retry, onOther = onPick)
        Screen.Ready -> {
            val editing = model.editing
            when {
                editing != null -> Editor(model, editing)
                model.account != null -> Operations(model, snackbar)
                else -> Accounts(model, snackbar, onPick)
            }
            if (model.offerBiometric) {
                AlertDialog(
                    onDismissRequest = { model.enableBiometric(false) },
                    icon = { Icon(Icons.Fingerprint, contentDescription = null) },
                    title = { Text("Ouvrir avec l'empreinte ?") },
                    text = { Text("La prochaine fois, votre empreinte digitale suffira pour ouvrir ce fichier.") },
                    confirmButton = { TextButton(onClick = { model.enableBiometric(true) }) { Text("Activer") } },
                    dismissButton = { TextButton(onClick = { model.enableBiometric(false) }) { Text("Plus tard") } },
                )
            }
        }
    }
}

/** Page centrée, lisible aussi sur tablette. */
@Composable
private fun Centered(content: @Composable () -> Unit) {
    Box(
        Modifier
            .fillMaxSize()
            .padding(24.dp),
        contentAlignment = Alignment.Center,
    ) {
        Column(
            Modifier.widthIn(max = 420.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) { content() }
    }
}

@Composable
private fun Welcome(onPick: () -> Unit) = Scaffold { padding ->
    Box(Modifier.padding(padding)) {
        Centered {
            Text("BankPerfect", style = MaterialTheme.typography.headlineMedium, fontWeight = FontWeight.SemiBold)
            Text(
                "Choisissez votre fichier de comptes (.bp) dans Dropbox, Google Drive, OneDrive… " +
                    "Il reste synchronisé avec BankPerfect sur ordinateur.",
                textAlign = TextAlign.Center,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Button(onClick = onPick) { Text("Choisir un fichier de comptes") }
        }
    }
}

@Composable
private fun Loading(message: String) = Scaffold { padding ->
    Box(Modifier.padding(padding)) {
        Centered {
            CircularProgressIndicator()
            if (message.isNotEmpty()) Text(message, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

@Composable
private fun Password(
    screen: Screen.Password,
    onUnlock: (String) -> Unit,
    onBiometric: () -> Unit,
    onOther: () -> Unit,
) = Scaffold { padding ->
    // Vidé après un mot de passe refusé.
    var password by rememberSaveable(screen.attempt) { mutableStateOf("") }
    var visible by rememberSaveable { mutableStateOf(false) }
    val focus = remember { FocusRequester() }
    // Une seule demande de focus, à l'affichage : le clavier s'ouvre (sauf
    // si l'empreinte est proposée).
    LaunchedEffect(screen.attempt) { if (!screen.biometric) focus.requestFocus() }
    Box(Modifier.padding(padding)) {
        Centered {
            Text("Fichier protégé", style = MaterialTheme.typography.headlineSmall)
            if (screen.fileName.isNotEmpty()) {
                Text(screen.fileName, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            OutlinedTextField(
                value = password,
                onValueChange = { password = it },
                label = { Text("Mot de passe") },
                singleLine = true,
                isError = screen.error != null,
                supportingText = screen.error?.let { { Text(it) } },
                visualTransformation = if (visible) VisualTransformation.None else PasswordVisualTransformation(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password, imeAction = ImeAction.Done),
                keyboardActions = KeyboardActions(onDone = { if (password.isNotEmpty()) onUnlock(password) }),
                trailingIcon = {
                    TextButton(onClick = { visible = !visible }) { Text(if (visible) "Masquer" else "Afficher") }
                },
                modifier = Modifier
                    .fillMaxWidth()
                    .focusRequester(focus),
            )
            Button(
                onClick = { onUnlock(password) },
                enabled = password.isNotEmpty(),
                modifier = Modifier.fillMaxWidth(),
            ) { Text("Ouvrir") }
            if (screen.biometric) {
                OutlinedButton(onClick = onBiometric, modifier = Modifier.fillMaxWidth()) {
                    Icon(Icons.Fingerprint, contentDescription = null, modifier = Modifier.size(20.dp))
                    Spacer(Modifier.width(8.dp))
                    Text("Utiliser l'empreinte")
                }
            }
            TextButton(onClick = onOther) { Text("Choisir un autre fichier") }
        }
    }
}

@Composable
private fun Failure(message: String, onRetry: () -> Unit, onOther: () -> Unit) = Scaffold { padding ->
    Box(Modifier.padding(padding)) {
        Centered {
            Text("Ouverture impossible", style = MaterialTheme.typography.headlineSmall)
            Text(message, textAlign = TextAlign.Center, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Button(onClick = onRetry) { Text("Réessayer") }
            OutlinedButton(onClick = onOther) { Text("Choisir un autre fichier") }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun Accounts(model: Model, snackbar: SnackbarHostState, onPick: () -> Unit) {
    var menu by remember { mutableStateOf(false) }
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(model.title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = MaterialTheme.colorScheme.background),
                actions = {
                    SavingIndicator(model.saving)
                    IconButton(onClick = model::refresh) { Icon(Icons.Refresh, contentDescription = "Actualiser") }
                    Box {
                        IconButton(onClick = { menu = true }) { Icon(Icons.More, contentDescription = "Menu") }
                        DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                            if (model.biometricEnabled()) {
                                DropdownMenuItem(
                                    text = { Text("Ne plus ouvrir par empreinte") },
                                    onClick = { menu = false; model.disableBiometric() },
                                )
                            }
                            DropdownMenuItem(
                                text = { Text("Choisir un autre fichier") },
                                onClick = { menu = false; onPick() },
                            )
                            DropdownMenuItem(
                                text = { Text("Oublier ce fichier") },
                                onClick = { menu = false; model.forget() },
                            )
                        }
                    }
                },
            )
        },
        snackbarHost = { SnackbarHost(snackbar) },
    ) { padding ->
        LazyColumn(
            contentPadding = PaddingValues(
                start = 16.dp,
                end = 16.dp,
                top = padding.calculateTopPadding() + 8.dp,
                bottom = padding.calculateBottomPadding() + 16.dp,
            ),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            items(model.accounts, key = { it.index.toInt() }) { account ->
                AccountCard(account) { model.openAccount(account.index.toInt()) }
            }
        }
    }
}

/** Petit indicateur d'écriture dans le cloud. */
@Composable
fun SavingIndicator(saving: Boolean) {
    if (saving) {
        CircularProgressIndicator(
            modifier = Modifier
                .padding(horizontal = 8.dp)
                .size(18.dp),
            strokeWidth = 2.dp,
        )
    }
}

@Composable
private fun AccountCard(account: AccountSummary, onClick: () -> Unit) {
    val money = LocalMoney.current
    Card(
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer),
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick),
    ) {
        Row(
            Modifier.padding(horizontal = 16.dp, vertical = 14.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f)) {
                Text(
                    account.name,
                    style = MaterialTheme.typography.titleMedium,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    color = if (account.disabled) {
                        MaterialTheme.colorScheme.onSurfaceVariant
                    } else {
                        MaterialTheme.colorScheme.onSurface
                    },
                )
                val pending = account.pending.toInt()
                val sub = listOfNotNull(
                    account.bank.takeIf { it.isNotEmpty() },
                    if (pending > 0) "$pending à pointer" else null,
                ).joinToString(" · ")
                if (sub.isNotEmpty()) {
                    Text(
                        sub,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
            Spacer(Modifier.width(12.dp))
            Column(horizontalAlignment = Alignment.End) {
                Text(
                    account.balance,
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold,
                    color = if (account.negative) money.negative else MaterialTheme.colorScheme.onSurface,
                )
                Text(
                    "En banque : ${account.pointed}",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}
