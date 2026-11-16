package net.chelly.bankperfect

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.viewModels
import net.chelly.bankperfect.ui.BankPerfectTheme
import net.chelly.bankperfect.ui.Root

class MainActivity : ComponentActivity() {
    private val model: Model by viewModels()

    /** Sélecteur de fichiers d'Android : tous types, un `.bp` n'a pas de type connu. */
    private val picker = registerForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        uri?.let(model::choose)
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
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
}
