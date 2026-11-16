package net.chelly.bankperfect.ui

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
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
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
        is Screen.Password -> Password(screen, onUnlock = model::unlock, onOther = onPick)
        is Screen.Accounts -> Accounts(
            screen,
            snackbar,
            onRefresh = model::refresh,
            onOther = onPick,
            onForget = model::forget,
        )
        is Screen.Failure -> Failure(screen.message, onRetry = model::retry, onOther = onPick)
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
private fun Password(screen: Screen.Password, onUnlock: (String) -> Unit, onOther: () -> Unit) =
    Scaffold { padding ->
        // Vidé après un mot de passe refusé (nouvel écran).
        var password by rememberSaveable(screen) { mutableStateOf("") }
        var visible by rememberSaveable { mutableStateOf(false) }
        val focus = remember { FocusRequester() }
        // Une seule demande de focus, à l'affichage : le clavier s'ouvre.
        LaunchedEffect(Unit) { focus.requestFocus() }
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
private fun Accounts(
    screen: Screen.Accounts,
    snackbar: SnackbarHostState,
    onRefresh: () -> Unit,
    onOther: () -> Unit,
    onForget: () -> Unit,
) {
    var menu by remember { mutableStateOf(false) }
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(screen.title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = MaterialTheme.colorScheme.background),
                actions = {
                    TextButton(onClick = onRefresh) { Text("Actualiser") }
                    Box {
                        TextButton(onClick = { menu = true }) { Text("⋮", style = MaterialTheme.typography.titleLarge) }
                        DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                            DropdownMenuItem(
                                text = { Text("Choisir un autre fichier") },
                                onClick = { menu = false; onOther() },
                            )
                            DropdownMenuItem(
                                text = { Text("Oublier ce fichier") },
                                onClick = { menu = false; onForget() },
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
            items(screen.accounts, key = { it.index.toInt() }) { account -> AccountCard(account) }
        }
    }
}

@Composable
private fun AccountCard(account: AccountSummary) {
    val money = LocalMoney.current
    Card(
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer),
        modifier = Modifier.fillMaxWidth(),
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
                if (account.bank.isNotEmpty()) {
                    Text(
                        account.bank,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
            Spacer(Modifier.height(0.dp).widthIn(min = 12.dp))
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
