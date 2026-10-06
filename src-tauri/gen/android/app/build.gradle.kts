import java.util.Properties
import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("rust")
}

val tauriProperties = Properties().apply {
    val propFile = file("tauri.properties")
    if (propFile.exists()) {
        propFile.inputStream().use { load(it) }
    }
}

// Release signing reads gen/android/keystore.properties (untracked; see
// docs/maintenance.md → "Android release keystore"). Locally it points at the
// developer's keystore; CI writes it from repository secrets. Without the file
// the release build produces an unsigned APK/AAB — installable nowhere, but
// debug builds are unaffected.
val keystorePropertiesFile = rootProject.file("keystore.properties")
val keystoreProperties = Properties().apply {
    if (keystorePropertiesFile.exists()) {
        keystorePropertiesFile.inputStream().use { load(it) }
    }
}

android {
    compileSdk = 37
    namespace = "org.terrazgo.app"
    defaultConfig {
        manifestPlaceholders["usesCleartextTraffic"] = "false"
        applicationId = "org.terrazgo.app"
        minSdk = 24
        targetSdk = 37
        versionCode = tauriProperties.getProperty("tauri.android.versionCode", "1").toInt()
        versionName = tauriProperties.getProperty("tauri.android.versionName", "1.0")
    }
    signingConfigs {
        create("release") {
            if (keystorePropertiesFile.exists()) {
                keyAlias = keystoreProperties.getProperty("keyAlias")
                keyPassword = keystoreProperties.getProperty("password")
                storeFile = file(keystoreProperties.getProperty("storeFile"))
                storePassword = keystoreProperties.getProperty("password")
            }
        }
    }
    buildTypes {
        getByName("debug") {
            manifestPlaceholders["usesCleartextTraffic"] = "true"
            isDebuggable = true
            isJniDebuggable = true
            isMinifyEnabled = false
            packaging {
                jniLibs.keepDebugSymbols.add("*/arm64-v8a/*.so")
                jniLibs.keepDebugSymbols.add("*/armeabi-v7a/*.so")
                jniLibs.keepDebugSymbols.add("*/x86/*.so")
                jniLibs.keepDebugSymbols.add("*/x86_64/*.so")
            }
        }
        getByName("release") {
            if (keystorePropertiesFile.exists()) {
                signingConfig = signingConfigs.getByName("release")
            }
            optimization {
               enable = true
            }
            proguardFiles(
                *fileTree(".") {
                  include("**/*.pro")
                  exclude("build/**")
                }.files.toTypedArray()
            )
        }
    }
    // Stays at 1.8, and the release log's "source value 8 is obsolete" warnings
    // are NOT ours to fix (measured 2026-09-04, still so in Tauri 2.12.1). They
    // come from javac, and this module has no Java at all — ten Kotlin files and
    // zero .java. The three lines are one javac invocation inside Tauri's own
    // Gradle modules, which build from the cargo registry
    // (tauri-<version>/mobile/android and each plugin's android/) and hardcode
    // VERSION_1_8 there.
    //
    // Raising this module to 17 was tried and verified to change nothing: the
    // build was clean and the warnings identical. Reverted rather than kept,
    // because it only diverged from the scaffold. `gradle.properties`'
    // suppressSourceTargetDeprecationWarning would hide the warnings instead of
    // fixing them, and they are a real signal about upstream — leave them
    // visible until a Tauri release moves them.
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
    }
    buildFeatures {
        buildConfig = true
    }
}

kotlin {
    compilerOptions {
        jvmTarget = JvmTarget.JVM_1_8
    }
}

rust {
    rootDirRel = "../../../"
}

// rustls-platform-verifier's Kotlin half (org.rustls.platformverifier.
// CertificateVerifier): the Rust side calls it over JNI to verify TLS
// certificates against the Android trust store. It comes from the crate's
// GitHub-hosted Maven repository, at exactly the version of
// `rustls-platform-verifier-android` in Cargo.lock: a version that differs
// from the Rust half's can crash at runtime. The snippet is the crate's own
// README's (0.7.1), read once per configuration and friendly to the
// configuration cache.
repositories {
    maven {
        url = uri("https://github.com/rustls/rustls-platform-verifier/raw/maven-archive/android-release-support/maven/")
    }
}

abstract class RustlsVersion : ValueSource<String, RustlsVersion.Params> {
    interface Params : ValueSourceParameters {
        val lockFile: RegularFileProperty
    }

    companion object {
        const val CRATE_NAME = "rustls-platform-verifier-android"
    }

    override fun obtain(): String {
        val version = parameters.lockFile.get().asFile.readLines().let { lines ->
            val nameIdx = lines.indexOfFirst { it.trim() == "name = \"$CRATE_NAME\"" }
            if (nameIdx < 0) {
                null
            } else {
                lines.drop(nameIdx + 1)
                    .firstOrNull { it.trimStart().startsWith("version = ") }
                    ?.substringAfter('"', "")
                    ?.substringBefore('"', "")
                    ?.takeIf { it.isNotEmpty() }
            }
        }
        return version ?: error("$CRATE_NAME not found in Cargo.lock")
    }
}

val rustlsPlatformVerifierVersion = providers.of(RustlsVersion::class.java) {
    parameters.lockFile.set(layout.projectDirectory.file("../../../../Cargo.lock"))
}

configurations.configureEach {
    resolutionStrategy.eachDependency {
        if (requested.group == "org.rustls" && requested.name == "rustls-platform-verifier") {
            useVersion(rustlsPlatformVerifierVersion.get())
            because("native component version must be identical to version of ${RustlsVersion.CRATE_NAME}")
        }
    }
}

dependencies {
    // Its version comes from Cargo.lock, by the resolution rule above.
    implementation("org.rustls:rustls-platform-verifier")
    implementation("androidx.webkit:webkit:1.14.0")
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation("androidx.activity:activity-ktx:1.10.1")
    implementation("com.google.android.material:material:1.12.0")
    implementation("androidx.lifecycle:lifecycle-process:2.10.0")
    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test.ext:junit:1.1.4")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.5.0")
}

apply(from = file("tauri.build.gradle.kts"))