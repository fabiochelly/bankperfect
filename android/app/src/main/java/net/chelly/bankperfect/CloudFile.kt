package net.chelly.bankperfect

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.OpenableColumns

/**
 * Le fichier de comptes choisi dans le sélecteur de fichiers d'Android :
 * Dropbox, Google Drive, OneDrive ou toute application qui s'y intègre.
 * L'accès accordé est conservé d'un lancement à l'autre.
 */
class CloudFile(private val context: Context) {
    private val prefs = context.getSharedPreferences("fichier", Context.MODE_PRIVATE)

    var uri: Uri?
        get() = prefs.getString("uri", null)?.let(Uri::parse)
        private set(value) = prefs.edit().putString("uri", value?.toString()).apply()

    /** Retient le fichier choisi et garde le droit de le lire et de l'écrire. */
    fun choose(uri: Uri) {
        val flags = Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION
        context.contentResolver.takePersistableUriPermission(uri, flags)
        this.uri?.takeIf { it != uri }?.let { old ->
            runCatching { context.contentResolver.releasePersistableUriPermission(old, flags) }
        }
        this.uri = uri
    }

    fun forget() {
        uri?.let { old ->
            val flags = Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION
            runCatching { context.contentResolver.releasePersistableUriPermission(old, flags) }
        }
        uri = null
    }

    /** Nom affiché du fichier (`Comptes.bp`). */
    fun name(): String? = uri?.let { uri ->
        runCatching {
            context.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
                ?.use { cursor -> if (cursor.moveToFirst()) cursor.getString(0) else null }
        }.getOrNull()
    }

    /** Contenu actuel du fichier. Bloquant : à appeler hors du fil principal. */
    fun read(): ByteArray {
        val uri = uri ?: error("Aucun fichier choisi")
        return context.contentResolver.openInputStream(uri)?.use { it.readBytes() }
            ?: error("Fichier illisible")
    }

    /** Remplace le contenu du fichier. Bloquant : à appeler hors du fil principal. */
    fun write(bytes: ByteArray) {
        val uri = uri ?: error("Aucun fichier choisi")
        // « wt » tronque l'ancien contenu ; certains fournisseurs ne
        // connaissent que « w », qui remplace aussi le fichier chez eux.
        val stream = runCatching { context.contentResolver.openOutputStream(uri, "wt") }.getOrNull()
            ?: context.contentResolver.openOutputStream(uri, "w")
            ?: error("Fichier impossible à écrire")
        stream.use { it.write(bytes) }
    }
}
