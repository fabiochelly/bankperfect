package net.chelly.bankperfect

import android.app.Application
import android.net.Uri
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import net.chelly.bankperfect.core.AccountSummary
import net.chelly.bankperfect.core.BpException
import net.chelly.bankperfect.core.Session
import net.chelly.bankperfect.core.SyncReport
import net.chelly.bankperfect.core.requiresPassword

/** Ce que montre l'application. */
sealed interface Screen {
    /** Aucun fichier choisi. */
    data object Welcome : Screen

    data class Loading(val message: String) : Screen

    data class Password(val fileName: String, val error: String? = null) : Screen

    data class Accounts(val title: String, val accounts: List<AccountSummary>) : Screen

    data class Failure(val message: String) : Screen
}

/**
 * État de l'application et ordres de l'interface. Le travail (lecture du
 * fichier, mot de passe, fusion) se fait hors du fil principal ; un seul à la
 * fois, pour ne jamais lire et écrire le fichier en même temps. Survit aux
 * rotations de l'écran (le mot de passe n'est pas redemandé).
 */
class Model(application: Application) : AndroidViewModel(application) {
    private val file = CloudFile(application)

    var screen by mutableStateOf<Screen>(if (file.uri == null) Screen.Welcome else Screen.Loading(""))
        private set

    /** Message bref (modifications intégrées, conflits). */
    var notice by mutableStateOf<String?>(null)

    private var session: Session? = null
    private val busy = Mutex()

    /**
     * Lance un travail. Une erreur à l'ouverture remplace l'écran ; une erreur
     * d'actualisation ou d'enregistrement s'affiche en message, le fichier
     * restant ouvert.
     */
    private fun work(opening: Boolean = true, block: suspend () -> Unit) {
        viewModelScope.launch {
            busy.withLock {
                try {
                    block()
                } catch (e: SecurityException) {
                    // Accès retiré par le fournisseur (fichier remplacé,
                    // application cloud réinstallée…).
                    screen = Screen.Failure(
                        "BankPerfect n'a plus accès à ce fichier. Choisissez-le à nouveau.",
                    )
                } catch (e: Exception) {
                    val message = e.message ?: e.toString()
                    if (opening || session == null) screen = Screen.Failure(message) else notice = message
                }
            }
        }
    }

    /** Au lancement : rouvre le dernier fichier choisi. */
    fun start() {
        if (file.uri != null && session == null) load()
    }

    /** Après une erreur : rouvre le fichier, ou l'actualise s'il est ouvert. */
    fun retry() {
        if (session == null) load() else refresh()
    }

    fun choose(uri: Uri) {
        file.choose(uri)
        session = null
        load()
    }

    fun forget() {
        file.forget()
        session = null
        screen = Screen.Welcome
    }

    private fun load() = work {
        screen = Screen.Loading("Lecture du fichier…")
        val bytes = withContext(Dispatchers.IO) { file.read() }
        if (withContext(Dispatchers.Default) { requiresPassword(bytes) }) {
            screen = Screen.Password(file.name() ?: "")
        } else {
            open(bytes, null)
        }
    }

    fun unlock(password: String) = work {
        screen = Screen.Loading("Ouverture…")
        val bytes = withContext(Dispatchers.IO) { file.read() }
        try {
            open(bytes, password)
        } catch (e: BpException.WrongPassword) {
            screen = Screen.Password(file.name() ?: "", "Mot de passe incorrect")
        }
    }

    private suspend fun open(bytes: ByteArray, password: String?) {
        val opened = withContext(Dispatchers.Default) { Session.open(bytes, password) }
        session = opened
        show(opened)
    }

    /** Au retour dans l'application : intègre ce qui a changé ailleurs. */
    fun refresh() {
        val current = session ?: return
        work(opening = false) {
            val bytes = withContext(Dispatchers.IO) { file.read() }
            val report = withContext(Dispatchers.Default) { current.refresh(bytes) }
            announce(report)
            show(current)
        }
    }

    /** Enregistre, après avoir fusionné la version du cloud. */
    fun save() {
        val current = session ?: return
        work(opening = false) {
            val remote = withContext(Dispatchers.IO) { runCatching { file.read() }.getOrNull() }
            val result = withContext(Dispatchers.Default) { current.save(remote) }
            withContext(Dispatchers.IO) { file.write(result.bytes) }
            current.saved()
            announce(result.report)
            show(current)
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

    private suspend fun show(session: Session) {
        val (title, accounts) = withContext(Dispatchers.Default) {
            session.title() to session.accounts()
        }
        screen = Screen.Accounts(title.ifEmpty { file.name() ?: "BankPerfect" }, accounts)
    }
}
