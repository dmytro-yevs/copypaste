import '../../../platform/android/android_quick_access_gateway.dart';

/// How copies made in other apps reach History on this Android device.
enum QuickAccessCaptureTier {
  /// Background capture granted through Shizuku or ADB saves every copy.
  full,

  /// The clips keyboard is the current input method and collects each copy.
  panelCollecting,

  /// The clips keyboard is turned on and saves the clipboard when it opens.
  panelCatchUp,

  /// Copies are saved from the app, a tile, the widget or Share only.
  manual;

  static QuickAccessCaptureTier of(AndroidQuickAccessState state) {
    if (state.backgroundCapture) return full;
    if (state.keyboardSelected) return panelCollecting;
    if (state.keyboardEnabled) return panelCatchUp;
    return manual;
  }
}
