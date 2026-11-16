import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
}

// Hors du dépôt : voir `android/make-keystore.sh`.
val signing = Properties().apply {
    val file = File(System.getProperty("user.home"), ".config/bankperfect/android-signing.properties")
    if (file.isFile) file.inputStream().use { load(it) }
}

// Même version que l'application de bureau ; le numéro de build Android est le
// nombre de commits (toujours croissant).
val version = rootDir.resolve("../Cargo.toml").readLines()
    .first { it.startsWith("version = ") }
    .substringAfter('"').substringBefore('"')
val commits = providers.exec { commandLine("git", "rev-list", "--count", "HEAD") }
    .standardOutput.asText.get().trim().toInt()

android {
    namespace = "net.chelly.bankperfect"
    compileSdk = 37
    ndkVersion = "30.0.16248370"

    defaultConfig {
        applicationId = "net.chelly.bankperfect"
        minSdk = 26
        targetSdk = 37
        versionCode = commits
        versionName = version
        ndk { abiFilters += listOf("arm64-v8a", "x86_64") }
    }

    signingConfigs {
        if (signing.isNotEmpty()) {
            create("release") {
                storeFile = file(signing.getProperty("storeFile"))
                storePassword = signing.getProperty("password")
                keyAlias = signing.getProperty("keyAlias")
                keyPassword = signing.getProperty("password")
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            signingConfig = signingConfigs.findByName("release") ?: signingConfigs["debug"]
        }
    }

    buildFeatures { compose = true }

    sourceSets["main"].apply {
        kotlin.directories.add(layout.buildDirectory.dir("generated/uniffi").get().asFile.path)
        jniLibs.directories.add(layout.buildDirectory.dir("generated/jniLibs").get().asFile.path)
    }

    packaging {
        // Le système charge les bibliothèques directement depuis l'APK.
        jniLibs.useLegacyPackaging = false
    }
}

val rust = tasks.register<Exec>("buildRust") {
    workingDir = rootDir
    commandLine("./build-rust.sh", layout.buildDirectory.dir("generated").get().asFile.path)
    inputs.dir("$rootDir/../crates/bp-core/src")
    inputs.dir("$rootDir/../crates/bp-mobile/src")
    inputs.file("$rootDir/../Cargo.lock")
    outputs.dir(layout.buildDirectory.dir("generated"))
}

tasks.named("preBuild") { dependsOn(rust) }

dependencies {
    val compose = platform("androidx.compose:compose-bom:2026.09.00")
    implementation(compose)
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.activity:activity-compose:1.13.0")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.11.0")
    implementation("androidx.biometric:biometric:1.1.0")
    implementation("net.java.dev.jna:jna:5.19.1@aar")
}
