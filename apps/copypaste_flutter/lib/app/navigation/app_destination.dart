import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../features/devices/device_presentation.dart';
import '../../l10n/l10n.dart';

/// The destinations available in the application shell.
enum AppDestination { history, devices, settings }

/// Shared presentation and accessibility data for an [AppDestination].
class AppNavigationDestination {
  const AppNavigationDestination({
    required this.destination,
    required this.icon,
    required this.shortcutKey,
  });

  final AppDestination destination;
  final IconData icon;

  /// Pressed with the desktop command modifier to open the destination.
  final LogicalKeyboardKey shortcutKey;

  String get label => switch (destination) {
    AppDestination.history => AppStrings.current.appDestinationHistory,
    AppDestination.devices => AppStrings.current.appDestinationDevices,
    AppDestination.settings => AppStrings.current.appDestinationSettings,
  };

  String semanticsLabel({required bool selected}) {
    return selected ? AppStrings.current.appDestinationSelected(label) : label;
  }
}

const List<AppNavigationDestination> appNavigationDestinations =
    <AppNavigationDestination>[
      AppNavigationDestination(
        destination: AppDestination.history,
        icon: LucideIcons.clipboardList,
        shortcutKey: LogicalKeyboardKey.digit1,
      ),
      AppNavigationDestination(
        destination: AppDestination.devices,
        icon: DevicePresentation.collectionIcon,
        shortcutKey: LogicalKeyboardKey.digit2,
      ),
      AppNavigationDestination(
        destination: AppDestination.settings,
        icon: LucideIcons.settings,
        shortcutKey: LogicalKeyboardKey.digit3,
      ),
    ];

extension AppDestinationNavigationData on AppDestination {
  AppNavigationDestination get navigationDestination {
    return appNavigationDestinations.singleWhere(
      (AppNavigationDestination item) => item.destination == this,
    );
  }
}
