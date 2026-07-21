import groovy.json.JsonSlurper
import java.util.Properties

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
    compileSdk = 36
    namespace = "org.terrazgo.app"
    defaultConfig {
        manifestPlaceholders["usesCleartextTraffic"] = "false"
        applicationId = "org.terrazgo.app"
        minSdk = 24
        targetSdk = 36
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
            packaging {                jniLibs.keepDebugSymbols.add("*/arm64-v8a/*.so")
                jniLibs.keepDebugSymbols.add("*/armeabi-v7a/*.so")
                jniLibs.keepDebugSymbols.add("*/x86/*.so")
                jniLibs.keepDebugSymbols.add("*/x86_64/*.so")
            }
        }
        getByName("release") {
            // Cleartext must stay allowed in release: certificate revocation
            // checks run inside this process, and CRL/OCSP endpoints are plain
            // http:// by design (CRLs are signed). With cleartext blocked,
            // Android's revocation fetch throws and rustls-platform-verifier
            // reports every CRL-only certificate (Let's Encrypt, Google Trust
            // Services — OpenFreeMap's CA) as "Revoked", blanking the base map.
            // The app's own traffic is unaffected: terrazgo-geo fetches HTTPS
            // allowlisted sources only, and the webview CSP is 'self' + geo:.
            manifestPlaceholders["usesCleartextTraffic"] = "true"
            if (keystorePropertiesFile.exists()) {
                signingConfig = signingConfigs.getByName("release")
            }
            isMinifyEnabled = true
            proguardFiles(
                *fileTree(".") { include("**/*.pro") }
                    .plus(getDefaultProguardFile("proguard-android-optimize.txt"))
                    .toList().toTypedArray()
            )
        }
    }
    kotlinOptions {
        jvmTarget = "1.8"
    }
    buildFeatures {
        buildConfig = true
    }
}

rust {
    rootDirRel = "../../../"
}

// rustls-platform-verifier's Kotlin half (org.rustls.platformverifier.
// CertificateVerifier): the Rust side calls it over JNI to verify TLS
// certificates against the Android trust store. The crate bundles the
// compiled .aar as a Maven repository inside its published sources, and
// `cargo metadata` locates the copy (and version) Cargo resolved — so the
// Kotlin component tracks crate upgrades automatically. The version must be
// explicit: the bundled repo has no maven-metadata.xml, which dynamic
// versions like latest.release need.
fun findRustlsPlatformVerifierAndroid(): Pair<String, String> {
    val metadataJson = providers.exec {
        workingDir = File(rootDir, "../../..")
        commandLine(
            "cargo", "metadata", "--format-version", "1",
            "--filter-platform", "aarch64-linux-android",
            "--manifest-path", "src-tauri/Cargo.toml"
        )
    }.standardOutput.asText.get()

    @Suppress("UNCHECKED_CAST")
    val packages = (JsonSlurper().parseText(metadataJson) as Map<String, Any>)
        .getValue("packages") as List<Map<String, Any>>
    val pkg = packages.first { it["name"] == "rustls-platform-verifier-android" }
    val mavenDir = File(File(pkg.getValue("manifest_path") as String).parentFile, "maven")
    return Pair(mavenDir.path, pkg.getValue("version") as String)
}

val rustlsVerifierAndroid = findRustlsPlatformVerifierAndroid()

repositories {
    maven {
        url = uri(rustlsVerifierAndroid.first)
        metadataSources {
            mavenPom()
            artifact()
        }
    }
}

dependencies {
    // Kept in lockstep with the Rust crate by the Maven repository above.
    implementation("rustls:rustls-platform-verifier:${rustlsVerifierAndroid.second}")
    implementation("androidx.webkit:webkit:1.14.0")
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation("androidx.activity:activity-ktx:1.10.1")
    implementation("com.google.android.material:material:1.12.0")
    implementation("androidx.lifecycle:lifecycle-process:2.10.0")
    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test.ext:junit:1.1.4")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.5.0")
}

apply(from = "tauri.build.gradle.kts")