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
        ?: listOf("arm64-v8a", "armeabi-v7a", "x86_64")
    val live = (project.findProperty("cargoNdkLive") as String?) != "false"

    val args = mutableListOf("cargo", "ndk")
    targets.forEach { args += listOf("-t", it) }
    args += listOf(
        "-o", "android/app/src/main/jniLibs",
        "build", "--release", "-p", "copypaste-android",
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
        versionCode = 6
        versionName = "0.5.1"
        // Instrumented (androidTest) runner for the cross-language crypto
        // conformance test (CryptoConformanceTest.kt). Runs on the emulator via
        // `./gradlew connectedDebugAndroidTest`.
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"))
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
    // Plain coordinate (no @aar): Gradle resolves JAR via POM for kotlinc compile
    // classpath + AAR .so resources at runtime. @aar bypasses POM resolution and
    // puts only the AAR on the classpath (dex-time only), causing "Unresolved
    // reference: Structure/Library/Native" in compileDebugKotlin.
    implementation("net.java.dev.jna:jna:5.14.0")

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
