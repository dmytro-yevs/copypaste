# ML Kit discovers these components from manifest metadata using reflection.
# Preserve registrar names and public constructors while allowing optimization.
-keep,allowoptimization class * implements com.google.firebase.components.ComponentRegistrar {
    public <init>();
}
