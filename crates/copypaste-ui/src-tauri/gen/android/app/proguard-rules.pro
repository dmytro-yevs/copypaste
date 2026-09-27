# Add project specific ProGuard rules here.
# You can control the set of applied configuration files using the
# proguardFiles setting in build.gradle.
#
# For more details, see
#   http://developer.android.com/guide/developing/tools/proguard.html

# A JNI symbol is spelled out of the class's package and name, so R8 renaming
# this one turns the keystore handover into an UnsatisfiedLinkError in release
# builds only — and then the app cannot open its own history.
-keep class com.copypaste.app.KeystoreContext { *; }

# Shizuku reflects this constructor in its privileged UserService process.
-keepclassmembers class com.copypaste.app.ShizukuSettingsService {
    public <init>();
}

# The initial-insets bridge is reached only from WebView JavaScript; R8 cannot
# infer that reachability. Keep the explicitly annotated, side-effect-free API.
-keep class com.copypaste.app.WebViewImeInsets$InsetBridge {
    @android.webkit.JavascriptInterface <methods>;
}

# Uncomment this to preserve the line number information for
# debugging stack traces.
#-keepattributes SourceFile,LineNumberTable

# If you keep the line number information, uncomment this to
# hide the original source file name.
#-renamesourcefileattribute SourceFile
