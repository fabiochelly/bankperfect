# JNA et UniFFI retrouvent ces classes par réflexion, depuis le code natif.
-keep class com.sun.jna.** { *; }
-keep class net.chelly.bankperfect.core.** { *; }
-dontwarn java.awt.**
