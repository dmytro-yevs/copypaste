import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.serialization")
    id("rust")
}

val tauriProperties = Properties().apply {
    val propFile = file("tauri.properties")
    if (propFile.exists()) {
        propFile.inputStream().use { load(it) }
    }
}

android {
    compileSdk = 36
    namespace = "com.copypaste.app"
    defaultConfig {
        manifestPlaceholders["usesCleartextTraffic"] = "false"
        applicationId = "com.copypaste.app"
        minSdk = 24
        targetSdk = 36
        versionCode = tauriProperties.getProperty("tauri.android.versionCode", "1").toInt()
        versionName = tauriProperties.getProperty("tauri.android.versionName", "1.0")
    }
    buildTypes {
        getByName("debug") {
            applicationIdSuffix = ".debug"
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
        // AGP 8 disables AIDL by default. We declare exactly one hidden
        // interface — the clip-changed callback — because a Binder stub cannot
        // be produced by reflection. See the .aidl for why it is the only one.
        aidl = true
    }
    testOptions {
        unitTests.isIncludeAndroidResources = true
    }
}

rust {
    rootDirRel = "../../../"
}

dependencies {
    // Tauri 2.11.5 pins vulnerable jackson-databind 2.15.3. This is the lowest
    // release outside the five advisories present on that runtime dependency.
    constraints {
        implementation("com.fasterxml.jackson.core:jackson-databind:2.18.9")
    }

    implementation("com.google.zxing:core:3.5.4")
    implementation("com.google.android.gms:play-services-code-scanner:16.1.0")

    // The typed bridge replaces hand-built JSONObject keys. The runtime jar is
    // the cost of making Kotlin's wire model executable instead of duplicating
    // it in Rust; 1.9.0 is the maintained line built for Kotlin 2.2.
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.9.0")

    // Rung 2. AGENTS.md rule 1: the maintained client library, not a
    // hand-rolled binder proxy — `ShizukuBinderWrapper` and
    // `SystemServiceHelper` are exactly the two pieces we would otherwise be
    // reimplementing against a hidden API. MIT, from RikkaApps.
    //
    // The tradeoff to state: both repos have been quiet since mid-2025, so our
    // best background-capture path depends on a third-party app with one
    // maintainer. Rung 0 does not depend on it and keeps working if it dies.
    implementation("dev.rikka.shizuku:api:13.1.5")
    implementation("dev.rikka.shizuku:provider:13.1.5")

    implementation("androidx.webkit:webkit:1.14.0")
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation("androidx.activity:activity-ktx:1.10.1")
    implementation("com.google.android.material:material:1.12.0")
    implementation("androidx.lifecycle:lifecycle-process:2.10.0")
    testImplementation("junit:junit:4.13.2")
    testImplementation("org.robolectric:robolectric:4.16.1")
    androidTestImplementation("androidx.test.ext:junit:1.1.4")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.5.0")
}

apply(from = "tauri.build.gradle.kts")

// The Android CI entry point assembles the debug APK. Keep the Kotlin fixture
// guard on that maintained surface instead of relying on a manual Gradle call.
tasks.matching { it.name.startsWith("assemble") && it.name.endsWith("Debug") }
    .configureEach {
        val variant = name.removePrefix("assemble").removeSuffix("Debug")
        finalizedBy(tasks.matching { it.name == "test${variant}DebugUnitTest" })
    }
