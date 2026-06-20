# ProGuard / R8 keep-rules for CopyPaste Android (release variant).
#
# WHY THIS FILE EXISTS
# --------------------
# The release build type sets `isMinifyEnabled = true`. Without these rules R8
# would rename/strip the JNA interop and the UniFFI-generated bindings, which
# rely on RUNTIME REFLECTION (class names, method names, and @Structure.FieldOrder
# field names) to bind to the native library `libcopypaste_android.so`. The
# result at runtime would be UnsatisfiedLinkError / JNA failures, and the app
# would silently fall back to its crypto STUB instead of real XChaCha20-Poly1305.
#
# Evidence (generated bindings):
#   android/app/src/main/java/com/copypaste/generated/uniffi/copypaste_android/copypaste_android.kt
#     - `package uniffi.copypaste_android;`
#     - imports com.sun.jna.{Library,IntegerType,Native,Pointer,Structure,Callback} and com.sun.jna.ptr.*
#     - `internal interface UniffiLib : Library` (Native.load resolves methods by name via reflection)
#     - 34 @JvmField fields inside @Structure.FieldOrder Structures (RustBuffer, ForeignBytes,
#       UniffiRustCallStatus, UniffiForeignFuture*) — JNA maps native struct layout by FIELD NAME.
#     - many `com.sun.jna.Callback` interfaces used as FFI continuations.
#
# These rules are intentionally CONSERVATIVE: correctness of the native crypto
# path outweighs a few KB of extra retained code.

# CopyPaste-hh3w: Preserve runtime annotations so JNA can read @Structure.FieldOrder
# on RustBuffer, ForeignBytes, UniffiRustCallStatus, and UniffiForeignFuture* at
# runtime. Without this R8 strips the annotation attributes and JNA cannot map the
# native struct layout by field name, causing struct corruption / UnsatisfiedLinkError.
-keepattributes *Annotation*, AnnotationDefault

# --- JNA core: keep all classes + members (reflection-driven native binding) ---
-keep class com.sun.jna.** { *; }
-keep class net.java.dev.jna.** { *; }
-keepclassmembers class * extends com.sun.jna.* { *; }

# JNA Structure subclasses: native struct layout is mapped by FIELD NAME via
# @Structure.FieldOrder. Renaming/removing fields corrupts the FFI ABI.
-keep class * extends com.sun.jna.Structure { *; }
-keepclassmembers class * extends com.sun.jna.Structure { *; }

# JNA Callback interfaces: invoked from native via reflection; keep names + methods.
-keep class * implements com.sun.jna.Callback { *; }
-keepclassmembers class * implements com.sun.jna.Callback { *; }

# JNA Library interfaces (e.g. UniffiLib): Native.load binds methods by name.
-keep interface * extends com.sun.jna.Library { *; }

# JNA references java.awt.* (desktop) which does not exist on Android. These are
# never reached at runtime on Android; silence the missing-class warnings so R8
# does not fail the build.
-dontwarn java.awt.**
-dontwarn com.sun.jna.**

# --- UniFFI generated bindings ---
# The generated Kotlin lives in package `uniffi.copypaste_android` (confirmed via
# the `package` declaration in copypaste_android.kt). Keep the whole package so
# class names, the UniffiLib interface, Structures, Callbacks, and the @JvmField
# struct fields all survive minification.
-keep class uniffi.copypaste_android.** { *; }
-keepclassmembers class uniffi.copypaste_android.** { *; }

# The bindings are placed on disk under com/copypaste/generated/uniffi/... — the
# on-disk path differs from the Kotlin package, so keep that namespace too in case
# any tooling-generated helper class lands under it.
-keep class com.copypaste.generated.uniffi.** { *; }
-keepclassmembers class com.copypaste.generated.uniffi.** { *; }

# Defensive: if UniFFI ever emits enums mapped over the FFI boundary, R8 must not
# strip the synthetic values()/valueOf() used by reflection-based (de)serialization.
-keepclassmembers enum uniffi.copypaste_android.** {
    public static **[] values();
    public static ** valueOf(java.lang.String);
}
