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

// AGP's host-only UTP configurations pin vulnerable Netty and Protobuf;
// limiting the force by configuration keeps :app runtime graphs untouched.
val utpUpgrades = arrayOf(
    "io.netty:netty-buffer:4.1.137.Final",
    "io.netty:netty-codec:4.1.137.Final",
    "io.netty:netty-codec-http:4.1.137.Final",
    "io.netty:netty-codec-http2:4.1.137.Final",
    "io.netty:netty-codec-socks:4.1.137.Final",
    "io.netty:netty-common:4.1.137.Final",
    "io.netty:netty-handler:4.1.137.Final",
    "io.netty:netty-handler-proxy:4.1.137.Final",
    "io.netty:netty-resolver:4.1.137.Final",
    "io.netty:netty-transport:4.1.137.Final",
    "io.netty:netty-transport-native-unix-common:4.1.137.Final",
    "com.google.protobuf:protobuf-java:3.25.5",
    "com.google.protobuf:protobuf-java-util:3.25.5",
    "com.google.protobuf:protobuf-kotlin:3.25.5",
)

gradle.allprojects {
    if (rootProject.name == "buildSrc") {
        configurations.configureEach {
            resolutionStrategy.force(*auditPins)
        }
    } else {
        configurations.configureEach {
            // UTP is audit-only. Matching its configurations keeps the APK's
            // compile and runtime dependency graphs unchanged.
            if (name.removePrefix("_").startsWith("internal-unified-test-platform") ||
                name.startsWith("unified-test-platform")
            ) {
                resolutionStrategy.force(*utpUpgrades)
            }
        }
    }
}
