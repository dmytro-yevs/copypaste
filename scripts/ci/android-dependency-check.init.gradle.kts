// CopyPaste-oc15, third attempt. Dependency-Check runs from the root
// buildscript scope, whose parent is buildSrc's exported scope, so a library
// on both classpaths loads parent-first and splits across two ClassLoaders:
// NoSuchMethodError when the parent is merely older, IllegalAccessError when
// the parent lacks the class but holds its package-private callees.
//
// commons-lang3 reached buildSrc as a transitive of the commons-compress pin
// below — 1.27.1 declares lang3 3.16.0, and Strings arrived in 3.17.0 — so pin
// every module the two classpaths share, not only the one that failed last.
// commons-codec resolves to 1.17.1 on both sides already.

// Applied with --init-script from the audit step alone, so the APK build keeps
// resolving AGP's own versions.
val auditPins = arrayOf(
    "org.apache.commons:commons-compress:1.27.1",
    "org.apache.commons:commons-lang3:3.20.0",
    "commons-io:commons-io:2.22.0",
    "com.google.guava:guava:33.6.0-jre",
)

gradle.allprojects {
    if (rootProject.name == "buildSrc") {
        configurations.configureEach {
            resolutionStrategy.force(*auditPins)
        }
    }
}
