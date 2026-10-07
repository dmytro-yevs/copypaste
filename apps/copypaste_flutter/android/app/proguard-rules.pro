# ML Kit discovers these components from manifest metadata using reflection.
# Preserve registrar names and public constructors while allowing optimization.
-keep,allowoptimization class * implements com.google.firebase.components.ComponentRegistrar {
    public <init>();
}

# Rust calls this upcall by its JVM name. It has no managed call site for R8
# to discover, so retain the interface and each concrete callback entry point.
-keep interface com.copypaste.app.CaptureCallback {
    public void run(long);
}
-keepclassmembers class * implements com.copypaste.app.CaptureCallback {
    public void run(long);
}
