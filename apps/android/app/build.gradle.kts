plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
}

android {
    namespace = "com.copypaste.android"

    // API 35 (Android 15). Worth knowing before changing it: 34 made
    // `foregroundServiceType` mandatory, and 35 put a rolling 6-hour-per-day
    // budget on `dataSync` services. `SyncService` is written for both — see
    // its source.
    compileSdk = 35

    defaultConfig {
        applicationId = "com.copypaste.android"

        // 26 (Android 8.0). Two hard floors sit just below it: the Android
        // Keystore's AES-GCM support that `DeviceSecret` depends on arrived in
        // 23, and `NotificationChannel`, which a foreground service needs,
        // arrived in 26.
        minSdk = 26
        targetSdk = 35

        versionCode = 1
        versionName = "2.0.0-alpha.1"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"

        ndk {
            // The four ABIs Play still accepts. `armeabi-v7a` and `x86` are
            // here for older hardware and for the emulator respectively; both
            // cost a copy of the Rust library in the bundle, which Play splits
            // per-device anyway.
            abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64", "x86")
        }
    }

    // Where `scripts/build-rust.sh` puts the cross-compiled `.so` files. Kept
    // out of the source tree and out of git: they are build output.
    sourceSets["main"].jniLibs.srcDirs("src/main/jniLibs")

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
        }
        debug {
            applicationIdSuffix = ".debug"
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    buildFeatures {
        compose = true
    }

    packaging {
        resources.excludes += "/META-INF/{AL2.0,LGPL2.1}"
    }
}

dependencies {
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.lifecycle.runtime.ktx)
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.androidx.lifecycle.viewmodel.compose)
    implementation(libs.androidx.navigation.compose)
    implementation(libs.kotlinx.coroutines.android)

    implementation(platform(libs.compose.bom))
    implementation(libs.compose.ui)
    implementation(libs.compose.ui.graphics)
    implementation(libs.compose.material3)
    implementation(libs.compose.material.icons.extended)
    implementation(libs.compose.ui.tooling.preview)
    debugImplementation(libs.compose.ui.tooling)

    // UniFFI's generated Kotlin binds the `.so` through JNA. The `@aar` suffix
    // is load-bearing: the plain jar carries no Android natives and the app
    // dies with UnsatisfiedLinkError the first time it touches the library.
    implementation(variantOf(libs.jna) { artifactType("aar") })

    testImplementation(libs.junit)
    testImplementation(libs.kotlinx.coroutines.test)
}
