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

    // Read the availability flag at configuration time into a plain local so the
    // doFirst closure below captures a Boolean — NOT a reference to the build
    // script object. Capturing the script-level `cargoNdkAvailable` lazy val
    // directly makes the closure hold a `this$0` script reference that Gradle's
    // configuration cache cannot serialize (it fails with
    // "getCargoNdkAvailable() because this$0 is null" on cache restore).
    val ndkAvailable = cargoNdkAvailable
    doFirst {
        if (!ndkAvailable) {
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
        // Version is TAG-authoritative at release time: CI passes the values
        // derived from the pushed git tag via Gradle properties
        //   -PversionName=<bare tag, e.g. 0.6.0>
        //   -PversionCode=<monotonic int derived from the tag>
        // (see .github/workflows/release.yml, Android job). The literals below are
        // only DEV defaults for local builds with no -P override; they MUST stay
        // monotonically non-decreasing relative to the last shipped release so a
        // local build never produces a lower versionCode than a published one.
        // versionCode must be a monotonically increasing integer; increment it
        // with every Play Store / sideload release regardless of version string.
        versionName = (project.findProperty("versionName") as String?) ?: "0.7.5"
        versionCode = (project.findProperty("versionCode") as String?)?.toInt() ?: 705
        // Instrumented (androidTest) runner for the cross-language crypto
        // conformance test (CryptoConformanceTest.kt). Runs on the emulator via
        // `./gradlew connectedDebugAndroidTest`.
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        ndk {
            abiFilters += listOf("arm64-v8a")
        }
    }

    signingConfigs {
        // CopyPaste-aebx: debug.keystore is NOT committed to the repo (gitignored).
        // Each developer must generate a local debug keystore before building:
        //
        //   keytool -genkey -v \
        //     -keystore android/app/debug.keystore \
        //     -alias androiddebugkey \
        //     -keyalg RSA -keysize 2048 -validity 10000 \
        //     -storepass android -keypass android \
        //     -dname "CN=Android Debug,O=Android,C=US"
        //
        // Gradle auto-generates a local debug keystore on demand (see the
        // getByName("debug") block below) — no committed keystore and no extra CI
        // step needed. For sideloaded/test installs the debug signing certificate
        // will differ per machine — this is expected and safe. Do NOT commit
        // keystores to version control.
        //
        // NOTE: Because each developer's debug.keystore differs, OTA updates between
        // debug builds signed on different machines will require uninstall+reinstall.
        // This is acceptable for development; production uses the CI release keystore.
        getByName("debug") {
            // CopyPaste-aebx: the debug keystore is gitignored, so generate a
            // standard one on demand (dev machine, CI, or fork) the first time a
            // build needs it. Keeps the build self-contained without tracking a
            // secret. Uses the conventional Android debug credentials.
            val debugKeystore = file("debug.keystore")
            if (!debugKeystore.exists()) {
                project.exec {
                    commandLine(
                        "keytool", "-genkeypair", "-v",
                        "-keystore", debugKeystore.absolutePath,
                        "-alias", "androiddebugkey",
                        "-keyalg", "RSA", "-keysize", "2048", "-validity", "10000",
                        "-storepass", "android", "-keypass", "android",
                        "-dname", "CN=Android Debug,O=Android,C=US",
                    )
                }
            }
            storeFile = debugKeystore
            storePassword = "android"
            keyAlias = "androiddebugkey"
            keyPassword = "android"
        }

        // Release signing — values supplied by CI from GitHub secrets and passed
        // through to Gradle as project properties (or environment variables):
        //   ANDROID_KEYSTORE_FILE     — path to the decoded keystore (.jks/.keystore)
        //   ANDROID_KEYSTORE_PASSWORD — keystore (store) password
        //   ANDROID_KEY_ALIAS         — signing key alias
        //   ANDROID_KEY_PASSWORD      — key password
        // CI decodes ANDROID_KEYSTORE_BASE64 to a file and points
        // ANDROID_KEYSTORE_FILE at it (see .github/workflows/release.yml).
        //
        // GRACEFUL ABSENCE: forks and local builds have no secrets. When the
        // keystore path/credentials are missing we DO NOT create this config; the
        // release buildType then falls back to debug-signing below so the build
        // still succeeds (debug-signed). Never hard-fail a secretless build.
        // Secret values are never echoed by Gradle.
        val releaseStoreFile = (project.findProperty("ANDROID_KEYSTORE_FILE") as String?)
            ?: System.getenv("ANDROID_KEYSTORE_FILE")
        val releaseStorePassword = (project.findProperty("ANDROID_KEYSTORE_PASSWORD") as String?)
            ?: System.getenv("ANDROID_KEYSTORE_PASSWORD")
        val releaseKeyAlias = (project.findProperty("ANDROID_KEY_ALIAS") as String?)
            ?: System.getenv("ANDROID_KEY_ALIAS")
        val releaseKeyPassword = (project.findProperty("ANDROID_KEY_PASSWORD") as String?)
            ?: System.getenv("ANDROID_KEY_PASSWORD")
        if (releaseStoreFile != null && file(releaseStoreFile).exists() &&
            releaseStorePassword != null && releaseKeyAlias != null && releaseKeyPassword != null
        ) {
            create("release") {
                storeFile = file(releaseStoreFile)
                storePassword = releaseStorePassword
                keyAlias = releaseKeyAlias
                keyPassword = releaseKeyPassword
            }
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
            // Sign the release variant with the "release" config when CI supplied
            // the keystore secrets (see signingConfigs above). When the release
            // config is ABSENT (forks, local builds, no secrets) fall back to the
            // committed debug keystore so `assembleRelease` still produces an
            // installable (debug-signed) APK instead of an unsigned/failed build.
            // The R8 keep-rules above protect the native crypto path either way.
            signingConfig = signingConfigs.findByName("release")
                ?: signingConfigs.getByName("debug")
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    // Allow JVM unit tests to call Android-stub classes (org.json.*, android.util.Log, etc.)
    // without crashing.  Tests that exercise real Android behaviour must run on-device via
    // androidTest.  Wire-format helpers in SupabaseRealtimeClient use org.json but contain
    // no Android-specific side-effects, so stub return values (null / 0) are safe here.
    testOptions {
        unitTests {
            isReturnDefaultValues = true
        }
    }
    kotlinOptions {
        jvmTarget = "17"
    }
    // Jetpack Compose (beta-bonus history / pair / settings screens).
    buildFeatures {
        compose = true
        // AGP 8.x defaults buildConfig to OFF. The About screen reads the app
        // version/build at runtime from BuildConfig.VERSION_NAME / VERSION_CODE
        // (generated from versionName / versionCode above), so the version shown
        // to the user can never drift from the manifest. Required for AboutScreen.
        buildConfig = true
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

    // OkHttp: WebSocket transport for Supabase Realtime (P1.1).
    // REST paths keep using HttpURLConnection — only WS uses OkHttp.
    // OkHttp's @aar is NOT used here (unlike JNA) because OkHttp ships its
    // classes in the plain JAR (no per-ABI native libs); the plain coordinate
    // resolves via POM so kotlinc gets the compile classpath without any
    // duplicate-class risk.  No clash with the JNA @aar path.
    implementation("com.squareup.okhttp3:okhttp:4.12.0")

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
    // org.json reference implementation: replaces Android stubs so JVM unit
    // tests that exercise SupabaseRealtimeClient wire-format helpers (JSONArray /
    // JSONObject) get real behaviour instead of null-returning stubs.
    // The Android runtime already provides org.json at runtime; this dep is
    // testImplementation only — zero impact on the APK.
    testImplementation("org.json:json:20240303")

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
