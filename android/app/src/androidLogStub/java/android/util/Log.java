package android.util;

/**
 * Minimal no-op android.util.Log shim (android-material3-redesign S2.5
 * Paparazzi-classpath workaround — see the "testDebugUnitTestPreExisting"
 * comment block in android/app/build.gradle.kts for the full rationale:
 * app.cash.paparazzi:1.3.4's plugin disables AGP's normal
 * isReturnDefaultValues "mockable android.jar" mechanism for the whole
 * module's test classpath, replacing it with the real platform android.jar
 * whose native Log methods throw UnsatisfiedLinkError outside a properly
 * bootstrapped Robolectric/layoutlib sandbox — confirmed upstream,
 * currently-unresolved: cashapp/paparazzi#1908/#1331/#1922).
 *
 * Placed FIRST on testDebugUnitTestPreExisting's classpath so the JVM
 * classloader resolves android.util.Log from here instead of the real
 * platform jar, restoring the previous (isReturnDefaultValues-equivalent)
 * "log calls are silent no-ops in JVM unit tests" behaviour for the
 * pre-existing test suite. Every method mirrors the real android.util.Log
 * signature set actually called from this module's production code
 * (verified via `rg -o "Log\.\w+\(" android/app/src/main/java`).
 */
public final class Log {
    public static final int VERBOSE = 2;
    public static final int DEBUG = 3;
    public static final int INFO = 4;
    public static final int WARN = 5;
    public static final int ERROR = 6;
    public static final int ASSERT = 7;

    private Log() {}

    public static int v(String tag, String msg) { return 0; }
    public static int v(String tag, String msg, Throwable tr) { return 0; }
    public static int d(String tag, String msg) { return 0; }
    public static int d(String tag, String msg, Throwable tr) { return 0; }
    public static int i(String tag, String msg) { return 0; }
    public static int i(String tag, String msg, Throwable tr) { return 0; }
    public static int w(String tag, String msg) { return 0; }
    public static int w(String tag, String msg, Throwable tr) { return 0; }
    public static int w(String tag, Throwable tr) { return 0; }
    public static int e(String tag, String msg) { return 0; }
    public static int e(String tag, String msg, Throwable tr) { return 0; }
    public static int wtf(String tag, String msg) { return 0; }
    public static int wtf(String tag, Throwable tr) { return 0; }
    public static int wtf(String tag, String msg, Throwable tr) { return 0; }
    public static boolean isLoggable(String tag, int level) { return false; }
    public static String getStackTraceString(Throwable tr) { return tr == null ? "" : tr.toString(); }
    public static int println(int priority, String tag, String msg) { return 0; }
}
