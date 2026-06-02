import org.gradle.api.tasks.Exec

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
}

// ---------------------------------------------------------------------------
// cargo-ndk: compile Rust .so libraries for Android targets
// ---------------------------------------------------------------------------
val cargoNdkAvailable: Boolean by lazy {
    try {
        val result = ProcessBuilder("cargo", "ndk", "--version")
            .redirectErrorStream(true)
            .start()
            .waitFor()
        result == 0
    } catch (_: Exception) {
        false
    }
}

// Root of the Cargo workspace (two levels above android/app/)
val workspaceRoot: String = rootProject.projectDir.parentFile.absolutePath

val buildCargoNdk by tasks.registering(Exec::class) {
    group = "build"
    description = "Compile copypaste-android into .so libraries via cargo-ndk"

    doFirst {
        if (!cargoNdkAvailable) {
            throw GradleException(
                """
                cargo-ndk is not installed or not on PATH.
                To install: cargo install cargo-ndk
                Then add Android NDK targets:
                  rustup target add aarch64-linux-android
                  rustup target add x86_64-linux-android
                Alternatively, build manually:
                  make android-so
                Or skip native libs for a UI-only build.
                """.trimIndent()
            )
        }
    }

    workingDir(workspaceRoot)
    // Beta W2.6: by default target arm64-v8a (modern devices) and armeabi-v7a
    // (legacy 32-bit), plus x86_64 for emulator use, with the `android-uniffi-live`
    // cargo feature so addClipboardItem/getHistoryCount do real DB I/O.
    //
    // The cross-language crypto conformance test only needs the pure
    // encrypt/decrypt FFI (no DB), and the CI/dev box may have only the
    // arm64-v8a Rust target installed. Two gradle properties keep that path lean:
    //   -PcargoNdkTargets=arm64-v8a   (comma-separated ABIs to build)
    //   -PcargoNdkLive=false          (drop the android-uniffi-live feature)
    // Example (matches the emulator AVD `copypaste_test`, ABI arm64-v8a):
    //   ./gradlew connectedDebugAndroidTest -PcargoNdkTargets=arm64-v8a -PcargoNdkLive=false
    val targets = (project.findProperty("cargoNdkTargets") as String?)
        ?.split(",")?.map { it.trim() }?.filter { it.isNotEmpty() }
        ?: listOf("arm64-v8a")
    val live = (project.findProperty("cargoNdkLive") as String?) != "false"

    val args = mutableListOf("cargo", "ndk")
    targets.forEach { args += listOf("-t", it) }
    args += listOf(
        "-o", "android/app/src/main/jniLibs",
        "build", "--profile", "release-size", "-p", "copypaste-android",
    )
    if (live) args += listOf("--features", "android-uniffi-live")
    commandLine(args)
}

// Wire cargo-ndk before every assembleDebug/assembleRelease
tasks.whenTaskAdded {
    if (name == "assembleDebug" || name == "assembleRelease") {
        dependsOn(buildCargoNdk)
    }
}

android {
    namespace = "com.copypaste.android"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.copypaste.android"
        minSdk = 26
        targetSdk = 35
        // MAINTENANCE: versionCode and versionName must be kept in sync with
        // [workspace.package] version in the root Cargo.toml. There is no
        // automated wiring — bump both manually on every release to avoid drift.
        // versionCode must be a monotonically increasing integer; increment it
        // with every Play Store / sideload release regardless of version string.
        versionCode = 8
        versionName = "0.5.3"
        // Instrumented (androidTest) runner for the cross-language crypto
        // conformance test (CryptoConformanceTest.kt). Runs on the emulator via
        // `./gradlew connectedDebugAndroidTest`.
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        ndk {
            abiFilters += listOf("arm64-v8a")
        }
    }

    signingConfigs {
        // Use a committed, stable debug keystore so every build (local and CI)
        // is signed with the SAME certificate. Without this, Android rejects
        // over-the-air updates with INSTALL_FAILED_UPDATE_INCOMPATIBLE because
        // Gradle auto-generates a fresh debug keystore on each machine/runner.
        //
        // THIS IS NOT A PRODUCTION SECRET — it is the standard debug key used
        // only for sideloaded/debug APKs. Do not use this keystore for Play Store
        // submissions; create a separate release keystore stored outside the repo.
        //
        // Credentials: storePassword=android, keyAlias=androiddebugkey, keyPassword=android
        // SHA-256: F6:23:D7:B2:FB:23:7D:F5:60:9E:7B:D7:A8:BB:FD:9D:7C:CF:A9:4C:AF:87:E8:D2:1D:3E:99:34:1F:CE:D9:53
        getByName("debug") {
            storeFile = file("debug.keystore")
            storePassword = "android"
            keyAlias = "androiddebugkey"
            keyPassword = "android"
        }
    }

    buildTypes {
        debug {
            signingConfig = signingConfigs.getByName("debug")
        }
        release {
            isMinifyEnabled = true
            // R8 keep-rules are REQUIRED here: the UniFFI bindings + JNA bind to
            // libcopypaste_android.so via runtime reflection (class/method names
            // and @Structure.FieldOrder field names). Without proguard-rules.pro,
            // R8 renames/strips them -> UnsatisfiedLinkError -> the app silently
            // falls back to the crypto STUB instead of real XChaCha20-Poly1305.
            // See android/app/proguard-rules.pro for the rules + rationale.
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
            // NOTE: a release APK is not actually shippable yet — no `signingConfig`
            // is configured (the project has no release keystore/secrets). CI builds
            // the DEBUG variant. These keep-rules exist so that IF/when a signed
            // release variant is built, the native crypto path survives minification.
            //
            // For LOCAL release testing WITHOUT secrets you can debug-sign the
            // release variant by adding (do NOT commit a real keystore):
            //   signingConfig = signingConfigs.getByName("debug")
            // then `./gradlew assembleRelease` and verify isNativeLibraryLoaded.
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }
    // Jetpack Compose (beta-bonus history / pair / settings screens).
    buildFeatures {
        compose = true
    }
    composeOptions {
        kotlinCompilerExtensionVersion = libs.versions.composeCompiler.get()
    }
    sourceSets {
        getByName("main") {
            // jniLibs path: arm64-v8a/.so placed here by build-android.sh (cargo-ndk).
            // If the .so is absent the app catches UnsatisfiedLinkError and runs in
            // stub mode — assembleDebug still succeeds (Gradle does not require the .so).
            jniLibs.srcDirs("src/main/jniLibs")
        }
    }
    packaging {
        resources {
            excludes += "/META-INF/{AL2.0,LGPL2.1}"
        }
        jniLibs {
            // JNA AAR bundles libjnidispatch.so for arm64-v8a/armeabi-v7a/x86_64;
            // our cargo-ndk also drops libcopypaste_android.so under jniLibs/arm64-v8a/.
            // pickFirsts prevents AGP DuplicateFilesException on mergeDebugNativeLibs.
            pickFirsts += listOf("**/libjnidispatch.so")
        }
    }
}

dependencies {
    implementation(libs.core.ktx)
    implementation(libs.appcompat)
    implementation(libs.material)
    implementation(libs.kotlinx.coroutines.android)
    implementation("androidx.lifecycle:lifecycle-viewmodel-ktx:2.7.0")
    // M7: Activity.lifecycleScope for auto-cancelled clipboard coroutines.
    implementation(libs.lifecycle.runtime.ktx)
    implementation("androidx.recyclerview:recyclerview:1.3.2")

    // Compose BOM — manages versions of all compose libs.
    implementation(platform(libs.compose.bom))
    implementation(libs.compose.ui)
    implementation(libs.compose.ui.tooling.preview)
    implementation(libs.compose.material3)
    implementation(libs.compose.material.icons)
    implementation(libs.compose.runtime.livedata)
    implementation(libs.activity.compose)
    implementation(libs.lifecycle.viewmodel.compose)
    debugImplementation(libs.compose.ui.tooling)

    // UniFFI generated Kotlin bindings use JNA for native interop.
    // We need TWO entries for JNA in the main app:
    //   1. Plain coordinate: Gradle resolves the JAR via POM so kotlinc has
    //      Structure/Library/Native on its compile classpath. @aar alone bypasses
    //      POM resolution and causes "Unresolved reference" in compileDebugKotlin.
    //   2. @aar: packages libjnidispatch.so (arm64-v8a / armeabi-v7a / x86_64)
    //      into the main APK so UniFFI's Native.load("copypaste_android") finds
    //      the JNA dispatch library at runtime.  Without this the main production
    //      app crashes with UnsatisfiedLinkError on first FFI call even when
    //      libcopypaste_android.so is present.
    // JNA @aar ONLY: the Android aar bundles the JNA classes (for the kotlinc
    // compile classpath) AND the per-ABI libjnidispatch.so (packaged into the
    // APK so UniFFI's Native.load resolves at runtime). Do NOT also depend on
    // the plain `jna:5.14.0` jar — having both puts the same com.sun.jna.*
    // classes on the classpath twice and D8 fails with "Duplicate class".
    implementation("net.java.dev.jna:jna:5.14.0@aar")

    // UniFFI Kotlin bindings are compiled as source (CopypasteBindings.kt).
    // Uncomment the line below only when using a separately-packaged bindings jar
    // (e.g. generated by an older uniffi-bindgen workflow):
    // implementation(files("libs/copypaste_android.jar"))

    // WorkManager: Supabase background poll worker
    implementation(libs.work.runtime.ktx)

    // ZXing: QR generation (core) for the pairing display + camera scanning
    // (android-embedded) so another device's QR can be read to pair.
    implementation(libs.zxing.core)
    implementation(libs.zxing.embedded)

    // JVM unit tests (src/test) — pure-Kotlin logic with no Android/FFI deps
    // (e.g. content-type normalization at the P2P sync boundary,
    // SupabaseClient.encodePayloadCt/decodePayloadCt bytea hex,
    // PairUtilsTest.formatScannedInfo, OemAutoStartHelper.detectManufacturer
    // mapping, and FgsSyncLoop backoff/interval math). Runs on the host JVM via
    // `./gradlew test` / `testDebugUnitTest`, no emulator needed.
    testImplementation("junit:junit:4.13.2")

    // Instrumented tests (androidTest) — cross-language crypto conformance.
    // AndroidX Test runner + ext-junit drive CryptoConformanceTest on a device
    // or emulator. JNA is already an `implementation` dep (loaded into the app
    // process), so the UniFFI bindings resolve the native lib at test time.
    androidTestImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test:runner:1.5.2")
    androidTestImplementation("androidx.test.ext:junit:1.1.5")
    // JNA's *own* native dispatch lib (libjnidispatch.so) ships only in the
    // @aar variant. The main app pulls the plain JAR (kotlinc classpath), so the
    // androidTest APK has no libjnidispatch.so and UniFFI's Native.load aborts
    // with UnsatisfiedLinkError. Adding the AAR here packages the per-ABI
    // libjnidispatch.so into the test build. pickFirsts (above) dedupes it.
    androidTestImplementation("net.java.dev.jna:jna:5.14.0@aar")
}
