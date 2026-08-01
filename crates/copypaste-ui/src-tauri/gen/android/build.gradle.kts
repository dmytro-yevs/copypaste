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
    }
}

allprojects {
    repositories {
        google()
        mavenCentral()
    }
}

apply(plugin = "org.owasp.dependencycheck")

configure<org.owasp.dependencycheck.gradle.extension.DependencyCheckExtension> {
    failBuildOnCVSS = 0.0F
    format = ReportGenerator.Format.ALL.toString()
}

tasks.register("clean").configure {
    delete("build")
}
