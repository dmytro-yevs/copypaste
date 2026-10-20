import 'package:flutter/foundation.dart';

import 'app_destination.dart';
import 'app_navigation_controller.dart';

/// Keeps settings-only presentation work aligned with visibility and selection.
class SettingsPresentationLifecycle {
  SettingsPresentationLifecycle({
    required this.navigation,
    required this.visible,
    required this.setActive,
  }) {
    navigation.addListener(refresh);
    visible.addListener(refresh);
    refresh();
  }

  final AppNavigationController navigation;
  final ValueListenable<bool> visible;
  final ValueChanged<bool> setActive;

  /// Reapply the current state when a runtime controller is replaced.
  void refresh() => setActive(
    visible.value && navigation.selectedDestination == AppDestination.settings,
  );

  void dispose() {
    navigation.removeListener(refresh);
    visible.removeListener(refresh);
  }
}
