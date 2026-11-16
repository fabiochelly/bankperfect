package net.chelly.bankperfect

import android.app.Application
import android.net.Uri
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import javax.crypto.Cipher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import net.chelly.bankperfect.core.AccountSummary
import net.chelly.bankperfect.core.AccountView
import net.chelly.bankperfect.core.BpException
import net.chelly.bankperfect.core.CategoryGuess
import net.chelly.bankperfect.core.CategoryItem
import net.chelly.bankperfect.core.Dashboard
import net.chelly.bankperfect.core.MonthReport
import net.chelly.bankperfect.core.RecentItem
import net.chelly.bankperfect.core.Draft
import net.chelly.bankperfect.core.ModeItem
import net.chelly.bankperfect.core.PayeeChoice
import net.chelly.bankperfect.core.Session
import net.chelly.bankperfect.core.SyncReport
import net.chelly.bankperfect.core.TackView
import net.chelly.bankperfect.core.requiresPassword

/** Ce que montre l'application, hors fichier ouvert. */
sealed interface Screen {
    /** Aucun fichier choisi. */
    data object Welcome : Screen

    data class Loading(val message: String) : Screen

    /** `attempt` change à chaque refus, pour vider le champ. */
    data class Password(
        val fileName: String,
        val error: String? = null,
        val biometric: Boolean = false,
        val attempt: Int = 0,
    ) : Screen

    /** Fichier ouvert. */
    data object Ready : Screen

    data class Failure(val message: String) : Screen
}

/** Onglets de l'application, fichier ouvert. */
enum class Tab { HOME, ACCOUNTS, ANALYSIS }

/** Opération en cours de saisie ou de modification. */
data class Editing(val account: Int, val uid: ULong?, val draft: Draft)

/** Demande d'authentification par empreinte, satisfaite par l'activité. */
fun interface Prompter {
    fun authenticate(title: String, cipher: Cipher, onSuccess: (Cipher) -> Unit)
}

/**
 * État de l'application et ordres de l'interface. Le travail sur le fichier
 * (lecture, mot de passe, fusion, écriture) se fait hors du fil principal ;
 * un seul à la fois, pour ne jamais lire et écrire le fichier en même temps.
 * Survit aux rotations de l'écran (le mot de passe n'est pas redemandé).
 */
class Model(application: Application) : AndroidViewModel(application) {
    private val file = CloudFile(application)
    private val vault = KeyVault(application)

    var screen by mutableStateOf<Screen>(if (file.uri == null) Screen.Welcome else Screen.Loading(""))
        private set

    /** Message bref (modifications intégrées, conflits, erreurs). */
    var notice by mutableStateOf<String?>(null)

    var title by mutableStateOf("")
        private set
    var accounts by mutableStateOf<List<AccountSummary>>(emptyList())
        private set

    var tab by mutableStateOf(Tab.HOME)
    var dashboard by mutableStateOf<Dashboard?>(null)
        private set

    /** Analyse : mois affiché (0 = mois en cours, -1 = précédent…). */
    var reportOffset by mutableStateOf(0)
        private set
    var report by mutableStateOf<MonthReport?>(null)
        private set

    /** Actualisation demandée par l'utilisateur (geste « tirer »). */
    var refreshing by mutableStateOf(false)
        private set

    /** Compte ouvert (index réel) et ses opérations. */
    var account by mutableStateOf<Int?>(null)
        private set
    var view by mutableStateOf<AccountView?>(null)
        private set
    var search by mutableStateOf("")
        private set
    var tackView by mutableStateOf(TackView.ALL)
        private set

    var editing by mutableStateOf<Editing?>(null)
        private set

    /** Écriture dans le cloud en cours. */
    var saving by mutableStateOf(false)
        private set

    /** Proposer l'empreinte digitale après une ouverture par mot de passe. */
    var offerBiometric by mutableStateOf(false)
        private set

    /** Fourni par l'activité au premier plan. */
    var prompter: Prompter? = null

    private var session: Session? = null
    private val busy = Mutex()
    private var viewJob: Job? = null

    /**
     * Lance un travail sur le fichier. Une erreur à l'ouverture remplace
     * l'écran ; ensuite, elle s'affiche en message, le fichier restant ouvert.
     */
    private fun work(opening: Boolean = true, block: suspend () -> Unit) {
        viewModelScope.launch {
            busy.withLock {
                try {
                    block()
                } catch (e: SecurityException) {
                    // Accès retiré par le fournisseur (fichier remplacé,
                    // application cloud réinstallée…).
                    session = null
                    screen = Screen.Failure("BankPerfect n'a plus accès à ce fichier. Choisissez-le à nouveau.")
                } catch (e: Exception) {
                    val message = e.message ?: e.toString()
                    if (opening || session == null) screen = Screen.Failure(message) else notice = message
                }
            }
        }
    }

    // ── Ouverture ────────────────────────────────────────────────────────

    /** Au lancement : rouvre le dernier fichier choisi. */
    fun start() {
        if (file.uri != null && session == null && screen !is Screen.Password) load()
    }

    /** Après une erreur : rouvre le fichier, ou l'actualise s'il est ouvert. */
    fun retry() {
        if (session == null) load() else refresh()
    }

    fun choose(uri: Uri) {
        file.choose(uri)
        close()
        load()
    }

    fun forget() {
        file.uri?.let(vault::forget)
        file.forget()
        close()
        screen = Screen.Welcome
    }

    private fun close() {
        session = null
        tab = Tab.HOME
        dashboard = null
        report = null
        reportOffset = 0
        account = null
        view = null
        editing = null
        search = ""
        tackView = TackView.ALL
    }

    private fun load() = work {
        screen = Screen.Loading("Lecture du fichier…")
        val bytes = withContext(Dispatchers.IO) { file.read() }
        if (!withContext(Dispatchers.Default) { requiresPassword(bytes) }) {
            open(withContext(Dispatchers.Default) { Session.open(bytes, null) })
            return@work
        }
        val uri = file.uri
        val biometric = uri != null && vault.has(uri) && vault.available()
        screen = Screen.Password(file.name() ?: "", biometric = biometric)
        if (biometric) unlockWithBiometric()
    }

    fun unlock(password: String) = work {
        val previous = screen as? Screen.Password
        screen = Screen.Loading("Ouverture…")
        val bytes = withContext(Dispatchers.IO) { file.read() }
        try {
            val opened = withContext(Dispatchers.Default) { Session.open(bytes, password) }
            open(opened)
            val uri = file.uri
            offerBiometric = uri != null && !vault.has(uri) && vault.available() && opened.dataKey() != null
        } catch (e: BpException.WrongPassword) {
            screen = Screen.Password(
                file.name() ?: "",
                "Mot de passe incorrect",
                biometric = previous?.biometric ?: false,
                attempt = (previous?.attempt ?: 0) + 1,
            )
        }
    }

    /** Ouvre le fichier avec la clé gardée sous empreinte. */
    fun unlockWithBiometric() {
        val uri = file.uri ?: return
        val cipher = vault.decrypter(uri)
        if (cipher == null) {
            vault.forget(uri)
            screen = Screen.Password(file.name() ?: "")
            return
        }
        prompter?.authenticate("Ouvrir vos comptes", cipher) { authorized ->
            work {
                val key = vault.open(uri, authorized) ?: return@work
                screen = Screen.Loading("Ouverture…")
                val bytes = withContext(Dispatchers.IO) { file.read() }
                try {
                    open(withContext(Dispatchers.Default) { Session.openWithKey(bytes, key) })
                } catch (e: BpException.KeyRejected) {
                    vault.forget(uri)
                    screen = Screen.Password(file.name() ?: "", e.message)
                }
            }
        }
    }

    /** Réponse à la proposition d'ouvrir ce fichier par empreinte. */
    fun enableBiometric(accept: Boolean) {
        offerBiometric = false
        if (!accept) return
        val uri = file.uri ?: return
        val key = session?.dataKey() ?: return
        prompter?.authenticate("Activer l'empreinte digitale", vault.encrypter()) { cipher ->
            vault.store(uri, cipher, key)
            notice = "Ce fichier s'ouvrira avec votre empreinte"
        }
    }

    fun biometricEnabled(): Boolean = file.uri?.let(vault::has) ?: false

    fun disableBiometric() {
        file.uri?.let(vault::forget)
        notice = "Le mot de passe sera demandé à l'ouverture"
    }

    private suspend fun open(opened: Session) {
        session = opened
        showAccounts(opened)
        screen = Screen.Ready
    }

    // ── Synchronisation ──────────────────────────────────────────────────

    /** Au retour dans l'application : intègre ce qui a changé ailleurs. */
    fun refresh() {
        val current = session ?: return
        refreshing = true
        work(opening = false) {
            try {
                val bytes = withContext(Dispatchers.IO) { file.read() }
                val report = withContext(Dispatchers.Default) { current.refresh(bytes) }
                announce(report)
                if (report.incoming) {
                    showAccounts(current)
                    reloadView()
                }
                // Saisie faite hors ligne, pas encore écrite.
                if (current.isDirty()) save()
            } finally {
                refreshing = false
            }
        }
    }

    /** Enregistre dans le cloud, après avoir fusionné la version qui s'y trouve. */
    private fun save() {
        val current = session ?: return
        work(opening = false) {
            if (!current.isDirty()) return@work
            saving = true
            try {
                val remote = withContext(Dispatchers.IO) { runCatching { file.read() }.getOrNull() }
                val result = withContext(Dispatchers.Default) { current.save(remote) }
                withContext(Dispatchers.IO) { file.write(result.bytes) }
                current.saved()
                announce(result.report)
                if (result.report.incoming) {
                    showAccounts(current)
                    reloadView()
                }
            } finally {
                saving = false
            }
        }
    }

    private fun announce(report: SyncReport) {
        notice = when {
            report.conflicts.isNotEmpty() ->
                "Modifications faites ailleurs intégrées. Modifié des deux côtés :\n" +
                    report.conflicts.joinToString("\n") { "• $it" }
            report.incoming -> "Modifications faites ailleurs intégrées"
            else -> notice
        }
    }

    /** Comptes, tableau de bord et analyse à jour. */
    private suspend fun showAccounts(session: Session) {
        val offset = reportOffset
        val (name, list, dash) = withContext(Dispatchers.Default) {
            Triple(session.title(), session.accounts(), session.dashboard())
        }
        title = name.ifEmpty { file.name() ?: "BankPerfect" }
        accounts = list
        dashboard = dash
        report = withContext(Dispatchers.Default) { runCatching { session.monthReport(offset) }.getOrNull() }
    }

    fun changeReportOffset(delta: Int) {
        val current = session ?: return
        val offset = (reportOffset + delta).coerceAtMost(0)
        if (offset == reportOffset) return
        reportOffset = offset
        viewModelScope.launch {
            report = withContext(Dispatchers.Default) { runCatching { current.monthReport(offset) }.getOrNull() }
        }
    }

    // ── Consultation ─────────────────────────────────────────────────────

    fun openAccount(index: Int) {
        account = index
        search = ""
        tackView = TackView.ALL
        view = null
        reloadView()
    }

    fun closeAccount() {
        account = null
        view = null
    }

    fun changeSearch(text: String) {
        search = text
        reloadView()
    }

    fun changeTackView(value: TackView) {
        tackView = value
        reloadView()
    }

    /** Recalcule la liste affichée ; une frappe annule le calcul précédent. */
    private fun reloadView() {
        val current = session ?: return
        val index = account ?: return
        val text = search
        val tack = tackView
        viewJob?.cancel()
        viewJob = viewModelScope.launch {
            val result = withContext(Dispatchers.Default) {
                runCatching { current.operations(index.toUInt(), text, tack) }
            }
            result.onSuccess { view = it }.onFailure { notice = it.message }
        }
    }

    /** Après une modification locale : listes à jour, puis écriture. */
    private suspend fun changed(current: Session) {
        showAccounts(current)
        reloadView()
        save()
    }

    fun toggleTack(uid: ULong) {
        val current = session ?: return
        viewModelScope.launch {
            val result = withContext(Dispatchers.Default) { runCatching { current.toggleTack(uid) } }
            result.onSuccess { changed(current) }.onFailure { notice = it.message }
        }
    }

    // ── Saisie ───────────────────────────────────────────────────────────

    fun startNew() {
        val current = session ?: return
        val index = account ?: return
        editing = Editing(index, null, current.newDraft())
    }

    /** Saisie rapide depuis le tableau de bord, dans le compte par défaut. */
    fun quickAdd() {
        val current = session ?: return
        editing = Editing(current.defaultAccount().toInt(), null, current.newDraft())
    }

    /** Opération du tableau de bord, ouverte directement. */
    fun editRecent(item: RecentItem) {
        val current = session ?: return
        runCatching { current.draftOf(item.uid) }
            .onSuccess { editing = Editing(item.accountIndex.toInt(), item.uid, it) }
            .onFailure { notice = it.message }
    }

    fun startEdit(uid: ULong) {
        val current = session ?: return
        val index = account ?: return
        runCatching { current.draftOf(uid) }
            .onSuccess { editing = Editing(index, uid, it) }
            .onFailure { notice = it.message }
    }

    fun cancelEdit() {
        editing = null
    }

    /** Enregistre la saisie ; renvoie le message d'erreur à afficher, ou `null`. */
    suspend fun commit(draft: Draft): String? {
        val current = session ?: return "Fichier fermé"
        val edit = editing ?: return null
        val result = withContext(Dispatchers.Default) {
            runCatching { current.saveOperation(edit.account.toUInt(), edit.uid, draft) }
        }
        return result.fold(
            onSuccess = {
                editing = null
                changed(current)
                null
            },
            onFailure = { it.message },
        )
    }

    fun delete(uid: ULong) {
        val current = session ?: return
        viewModelScope.launch {
            val result = withContext(Dispatchers.Default) { runCatching { current.deleteOperation(uid) } }
            result.onSuccess {
                editing = null
                notice = "Opération supprimée"
                changed(current)
            }.onFailure { notice = it.message }
        }
    }

    fun modes(): List<ModeItem> = session?.modes() ?: emptyList()

    fun categories(): List<CategoryItem> = session?.categories() ?: emptyList()

    suspend fun payeeChoices(account: Int, text: String): List<PayeeChoice> {
        val current = session ?: return emptyList()
        return withContext(Dispatchers.Default) { current.payeeChoices(account.toUInt(), text) }
    }

    suspend fun fillFromPayee(account: Int, draft: Draft, choice: PayeeChoice): Draft {
        val current = session ?: return draft
        return withContext(Dispatchers.Default) { current.fillFromPayee(account.toUInt(), draft, choice) }
    }

    suspend fun guessCategory(draft: Draft): CategoryGuess? {
        val current = session ?: return null
        return withContext(Dispatchers.Default) { current.guessCategory(draft) }
    }

    suspend fun modeWarning(draft: Draft): String? {
        val current = session ?: return null
        return withContext(Dispatchers.Default) { current.modeWarning(draft) }
    }
}
