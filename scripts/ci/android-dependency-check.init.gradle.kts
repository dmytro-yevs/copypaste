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

// netty and protobuf reach this build only through the Unified Test Platform
// configurations AGP puts on every Android module, at versions compiled into
// AGP rather than declared in its POM — AGP 9.3.1 resolves the same
// netty 4.1.93/4.1.110, so no AGP upgrade moves them and forcing is the lever.
//
// 4.1.137.Final clears CVE-2023-34462, CVE-2023-44487, CVE-2024-29025,
// CVE-2024-47535, CVE-2025-24970 and CVE-2025-55163; 3.25.5 clears
// CVE-2024-7254 while staying on the contract gRPC 1.57/1.69 generate against.
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
            // By name, and only these names, so the force provably cannot change
            // what is packaged: no `:app` runtime or compile classpath resolves
            // either group. AGP 9 drops the leading underscore.
            if (name.removePrefix("_").startsWith("internal-unified-test-platform") ||
                name.startsWith("unified-test-platform")
            ) {
                resolutionStrategy.force(*utpUpgrades)
            }
        }
    }
}
