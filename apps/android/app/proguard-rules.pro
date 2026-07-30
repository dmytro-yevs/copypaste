# JNA reaches the native library reflectively, so R8 cannot see the uses and
# strips the classes it needs. Without these rules a minified release build
# fails at the first FFI call with a NoClassDefFoundError that looks nothing
# like the actual cause.
-dontwarn java.awt.**
-keep class com.sun.jna.** { *; }
-keep class * implements com.sun.jna.** { *; }

# The generated UniFFI bindings declare JNA structures and callbacks that are
# only ever instantiated from native code.
-keep class com.copypaste.ffi.** { *; }
