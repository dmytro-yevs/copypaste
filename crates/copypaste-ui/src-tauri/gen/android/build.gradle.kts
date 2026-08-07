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
    failBuildOnCVSS = 0.0F
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
