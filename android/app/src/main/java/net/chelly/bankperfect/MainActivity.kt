package net.chelly.bankperfect

import android.os.Bundle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.viewModels
import androidx.biometric.BiometricManager.Authenticators.BIOMETRIC_STRONG
import androidx.biometric.BiometricPrompt
import androidx.core.content.ContextCompat
import androidx.fragment.app.FragmentActivity
import net.chelly.bankperfect.ui.BankPerfectTheme
import net.chelly.bankperfect.ui.Root

/** `FragmentActivity` : la fenêtre d'empreinte digitale d'Android l'exige. */
class MainActivity : FragmentActivity() {
    private val model: Model by viewModels()

    /** Sélecteur de fichiers d'Android : tous types, un `.bp` n'a pas de type connu. */
    private val picker = registerForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        uri?.let(model::choose)
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        model.prompter = Prompter { title, cipher, onSuccess -> authenticate(title, cipher, onSuccess) }
        model.start()
        setContent {
            BankPerfectTheme {
                Root(model = model, onPick = { picker.launch(arrayOf("*/*")) })
            }
        }
    }

    override fun onRestart() {
        super.onRestart()
        model.refresh()
    }

    override fun onDestroy() {
        model.prompter = null
        super.onDestroy()
    }

    private fun authenticate(title: String, cipher: javax.crypto.Cipher, onSuccess: (javax.crypto.Cipher) -> Unit) {
        val prompt = BiometricPrompt(
            this,
            ContextCompat.getMainExecutor(this),
            object : BiometricPrompt.AuthenticationCallback() {
                override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) {
                    result.cryptoObject?.cipher?.let(onSuccess)
                }
            },
        )
        val info = BiometricPrompt.PromptInfo.Builder()
            .setTitle(title)
            .setNegativeButtonText("Mot de passe")
            .setAllowedAuthenticators(BIOMETRIC_STRONG)
            .build()
        prompt.authenticate(info, BiometricPrompt.CryptoObject(cipher))
    }
}
