import 'package:shadcn_flutter/shadcn_flutter.dart' show ThemeMode;

import '../shared/preference_choice.dart';
import 'theme/app_accent.dart';
import '../shared/preference_toggle.dart';

/// Device-local application preferences; never synced.
class DevicePreferences {
  DevicePreferences({
    PreferenceToggle? startInBackground,
    PreferenceToggle? confirmSingleDelete,
    PreferenceToggle? downloadUpdatesInBackground,
    PreferenceToggle? windowGlass,
    PreferenceChoice<ThemeMode>? themeMode,
    PreferenceChoice<AppAccent>? accent,
  }) : startInBackground =
           startInBackground ??
           PreferenceToggle(startInBackgroundKey, defaultValue: false),
       confirmSingleDelete =
           confirmSingleDelete ??
           PreferenceToggle(
             'history.confirm_single_delete',
             defaultValue: true,
           ),
       downloadUpdatesInBackground =
           downloadUpdatesInBackground ??
           PreferenceToggle(
             'updates.download_in_background',
             defaultValue: false,
           ),
       windowGlass =
           windowGlass ?? PreferenceToggle(windowGlassKey, defaultValue: true),
       themeMode = themeMode ?? themeModePreference(),
       accent = accent ?? accentPreference();

  /// Shared with the Quick Paste engine, which reads it on its own.
  static const themeModeKey = 'appearance.theme_mode';

  static PreferenceChoice<AppAccent> accentPreference() => PreferenceChoice(
    'appearance.accent',
    values: AppAccent.values,
    defaultValue: AppAccent.standard,
  );

  static PreferenceChoice<ThemeMode> themeModePreference() => PreferenceChoice(
    themeModeKey,
    values: ThemeMode.values,
    defaultValue: ThemeMode.system,
  );

  /// Read before the window exists, so it is shared with startup.
  static const startInBackgroundKey = 'startup.start_in_background';

  /// Shared with the Quick Paste engine, which reads it on its own.
  static const windowGlassKey = 'appearance.window_glass';

  /// A login launch leaves the window hidden. Desktop only.
  final PreferenceToggle startInBackground;

  /// Deleting one clip asks first. Bulk deletes always ask.
  final PreferenceToggle confirmSingleDelete;

  /// A found update is downloaded and verified before the user installs it.
  final PreferenceToggle downloadUpdatesInBackground;

  /// Desktop windows show the system glass material where it is available.
  final PreferenceToggle windowGlass;

  /// Light, dark, or following the system.
  final PreferenceChoice<ThemeMode> themeMode;

  /// The accent the UI highlights with.
  final PreferenceChoice<AppAccent> accent;

  Future<void> load() => Future.wait([
    startInBackground.load(),
    confirmSingleDelete.load(),
    downloadUpdatesInBackground.load(),
    windowGlass.load(),
    themeMode.load(),
    accent.load(),
  ]);

  void dispose() {
    startInBackground.dispose();
    confirmSingleDelete.dispose();
    downloadUpdatesInBackground.dispose();
    windowGlass.dispose();
    themeMode.dispose();
    accent.dispose();
  }
}
