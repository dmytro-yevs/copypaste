// CopyPaste-oc15, third attempt. Dependency-Check runs from the root
// buildscript scope, whose parent is buildSrc's exported scope, so a library
// on both classpaths loads parent-first and splits across two ClassLoaders:
// NoSuchMethodError when the parent is merely older, IllegalAccessError when
// the parent lacks the class but holds its package-private callees.
//
// commons-compress 1.27.1 brings lang3 3.16.0, while Strings arrived in 3.17.0,
// so pin every module shared by the two classpaths. commons-codec already
// resolves to 1.17.1 on both sides.

// Applied only by the audit step, so APK builds keep AGP's own versions.
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
