// The AOSP declaration, verbatim, so the compiler generates the Binder stub we
// have to subclass — `java.lang.reflect.Proxy` cannot stand in for one.
//
// This is the only hidden interface we declare. `IClipboard` is reached by
// reflection instead (see ShizukuClipboard), because its method signatures have
// gained parameters in several releases and a compiled-in AIDL would break on
// the next one. This one has been a single no-argument callback since it was
// introduced.
package android.content;

oneway interface IOnPrimaryClipChangedListener {
    void dispatchPrimaryClipChanged();
}
