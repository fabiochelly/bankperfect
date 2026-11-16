package net.chelly.bankperfect

import android.content.Context
import android.net.Uri
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyPermanentlyInvalidatedException
import android.security.keystore.KeyProperties
import android.util.Base64
import androidx.biometric.BiometricManager
import androidx.biometric.BiometricManager.Authenticators.BIOMETRIC_STRONG
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Déverrouillage par empreinte digitale : la clé du fichier (pas le mot de
 * passe) est chiffrée par une clé du coffre-fort d'Android, utilisable
 * seulement après une authentification biométrique forte. Une nouvelle
 * empreinte enregistrée sur le téléphone invalide la clé : le mot de passe
 * est alors redemandé.
 */
class KeyVault(private val context: Context) {
    private val prefs = context.getSharedPreferences("empreinte", Context.MODE_PRIVATE)

    fun available(): Boolean =
        BiometricManager.from(context).canAuthenticate(BIOMETRIC_STRONG) == BiometricManager.BIOMETRIC_SUCCESS

    fun has(uri: Uri): Boolean = prefs.contains(uri.toString())

    fun forget(uri: Uri) = prefs.edit().remove(uri.toString()).apply()

    /** Chiffrement à autoriser par l'empreinte, avant [store]. */
    fun encrypter(): Cipher {
        val cipher = Cipher.getInstance(TRANSFORMATION)
        try {
            cipher.init(Cipher.ENCRYPT_MODE, secretKey())
        } catch (e: KeyPermanentlyInvalidatedException) {
            deleteKey()
            cipher.init(Cipher.ENCRYPT_MODE, secretKey())
        }
        return cipher
    }

    /** Déchiffrement à autoriser par l'empreinte ; `null` si la clé n'est plus valable. */
    fun decrypter(uri: Uri): Cipher? {
        val (iv, _) = stored(uri) ?: return null
        return try {
            Cipher.getInstance(TRANSFORMATION).apply {
                init(Cipher.DECRYPT_MODE, secretKey(), GCMParameterSpec(128, iv))
            }
        } catch (e: KeyPermanentlyInvalidatedException) {
            prefs.edit().clear().apply()
            deleteKey()
            null
        }
    }

    fun store(uri: Uri, cipher: Cipher, key: ByteArray) {
        val sealed = cipher.doFinal(key)
        val value = encode(cipher.iv) + ":" + encode(sealed)
        prefs.edit().putString(uri.toString(), value).apply()
    }

    fun open(uri: Uri, cipher: Cipher): ByteArray? {
        val (_, sealed) = stored(uri) ?: return null
        return cipher.doFinal(sealed)
    }

    private fun stored(uri: Uri): Pair<ByteArray, ByteArray>? {
        val value = prefs.getString(uri.toString(), null) ?: return null
        val parts = value.split(':')
        if (parts.size != 2) return null
        return decode(parts[0]) to decode(parts[1])
    }

    private fun secretKey(): SecretKey {
        val store = KeyStore.getInstance(KEYSTORE).apply { load(null) }
        (store.getKey(ALIAS, null) as? SecretKey)?.let { return it }
        val spec = KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(256)
            .setUserAuthenticationRequired(true)
            .setInvalidatedByBiometricEnrollment(true)
            .build()
        return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE).run {
            init(spec)
            generateKey()
        }
    }

    private fun deleteKey() {
        KeyStore.getInstance(KEYSTORE).apply { load(null) }.deleteEntry(ALIAS)
    }

    private fun encode(bytes: ByteArray) = Base64.encodeToString(bytes, Base64.NO_WRAP)

    private fun decode(text: String) = Base64.decode(text, Base64.NO_WRAP)

    private companion object {
        const val KEYSTORE = "AndroidKeyStore"
        const val ALIAS = "bankperfect-fichiers"
        const val TRANSFORMATION = "AES/GCM/NoPadding"
    }
}
