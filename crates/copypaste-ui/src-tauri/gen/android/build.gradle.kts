import org.owasp.dependencycheck.reporting.ReportGenerator

buildscript {
    repositories {
        google()
        mavenCentral()
        maven(url = "https://plugins.gradle.org/m2/")
    }
    dependencies {
        classpath("com.android.tools.build:gradle:8.11.0")
        classpath("org.jetbrains.kotlin:kotlin-gradle-plugin:1.9.25")
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

// NVD answers 2000 CVEs a page, so a cold update is ~150 requests. Tokenless
// it allows 5 per rolling 30s and that update takes ~50 minutes; a key raises
// the limit to 50 per 30s and the same update takes ~11. Null is the supported
// state, not a failure — fork pull requests get no secrets and a clone has
// none, so both delays have to be values this build can actually run at.
val nvdApiKey: String? = System.getenv("NVD_API_KEY")?.takeIf { it.isNotBlank() }

configure<org.owasp.dependencycheck.gradle.extension.DependencyCheckExtension> {
    // ADR-0016. The threshold, the project filter and the suppression file are
    // one decision: at 7.0 without the filter the build still fails on netty
    // in AGP's test harness, and filtered without a threshold it still fails on
    // a Drupal XSS from 2009.
    failBuildOnCVSS = 7.0F
    scanProjects.set(listOf(":app"))
    // One release runtime classpath per ABI. The ABI list is generated into
    // app/tauri.build.gradle.kts, so read the names off the project instead of
    // pinning a copy that file is free to change. Empty is fatal: an audit that
    // silently resolves to nothing is worse than one that is noisy.
    scanConfigurations.set(provider {
        project(":app").configurations.names
            .filter { it.endsWith("ReleaseRuntimeClasspath") }
            .sorted()
            .also { require(it.isNotEmpty()) { ":app exposes no release runtime classpath" } }
    })
    suppressionFile = "$projectDir/dependency-check-suppressions.xml"
    // A rule that no longer suppresses anything is a rule nobody deleted.
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
