import org.owasp.dependencycheck.reporting.ReportGenerator

buildscript {
    repositories {
        google()
        mavenCentral()
        maven(url = "https://plugins.gradle.org/m2/")
    }
    dependencies {
        // The 8.x ceiling. AGP 9 is the only line that runs on Gradle 9, and it
        // needs android.newDsl=false and android.builtInKotlin=false because the
        // vendored Tauri Android projects apply org.jetbrains.kotlin.android
        // and set kotlinOptions. It also resolves the same netty as this does.
        classpath("com.android.tools.build:gradle:8.13.2")
        // Kotlin 2.3 rejects the String jvmTarget used by Tauri 2.11.5.
        classpath("org.jetbrains.kotlin:kotlin-gradle-plugin:2.2.21")
        classpath("org.jetbrains.kotlin:kotlin-serialization:2.2.21")
        // 13.0.0 keeps the vulnerable transitives and rebuilds the NVD database
        // from cold, consuming most of a tokenless runner's audit job.
        classpath("org.owasp:dependency-check-gradle:12.2.2")
        // CopyPaste-oc15: Dependency-Check 12.2.2 calls ZipFile.builder().
        classpath("org.apache.commons:commons-compress:1.27.1")
    }
}

allprojects {
    repositories {
        google()
        mavenCentral()
    }
}

apply(plugin = "org.owasp.dependencycheck")

val nvdApiKey = providers.environmentVariable("NVD_API_KEY").orNull
    ?.takeIf { it.isNotBlank() }

configure<org.owasp.dependencycheck.gradle.extension.DependencyCheckExtension> {
    failBuildOnCVSS = 7.0F
    scanProjects.set(listOf(":app"))
    scanConfigurations.set(provider {
        project(":app").configurations.names
            .filter { it.endsWith("ReleaseRuntimeClasspath") }
            .sorted()
            .also { require(it.isNotEmpty()) { ":app exposes no release runtime classpath" } }
    })
    suppressionFile = "$projectDir/dependency-check-suppressions.xml"
    failBuildOnUnusedSuppressionRule = true
    format = ReportGenerator.Format.ALL.toString()
    data.directory = "${System.getProperty("user.home")}/.gradle/dependency-check-data"
    nvd {
        apiKey = nvdApiKey
        delay = if (nvdApiKey == null) 16000 else 3500
        maxRetryCount = 20
        validForHours = 24
    }
}

tasks.register("clean").configure {
    delete("build")
}
