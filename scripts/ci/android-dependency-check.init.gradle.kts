// CopyPaste-oc15, second attempt. Dependency-Check 12.2.2 calls
// ZipFile.builder(), which arrived in commons-compress 1.26.0. Putting 1.27.1
// on the root buildscript classpath does not reach it: buildSrc pulls
// com.android.tools.build:gradle:8.11.0, whose commons-compress 1.21 is
// exported from a parent ClassLoaderScope and therefore wins parent-first,
// whatever the buildscript block asks for. Forcing it inside buildSrc's own
// resolution is the only place the version is still negotiable.
//
// Applied with --init-script from the audit step alone, so the APK build keeps
// resolving 1.21 exactly as it does today.
gradle.allprojects {
    if (rootProject.name == "buildSrc") {
        configurations.configureEach {
            resolutionStrategy.force("org.apache.commons:commons-compress:1.27.1")
        }
    }
}
