import 'package:shadcn_flutter/shadcn_flutter.dart';

/// The destinations available in the application shell.
enum AppDestination { history, devices, settings }

/// Shared presentation and accessibility data for an [AppDestination].
class AppNavigationDestination {
  const AppNavigationDestination({
    required this.destination,
    required this.label,
    required this.icon,
  });

  final AppDestination destination;
  final String label;
  final IconData icon;

  String semanticsLabel({required bool selected}) {
    return selected ? '$label, selected' : label;
  }
}

const List<AppNavigationDestination> appNavigationDestinations =
    <AppNavigationDestination>[
      AppNavigationDestination(
        destination: AppDestination.history,
        label: 'History',
        icon: LucideIcons.history,
      ),
      AppNavigationDestination(
        destination: AppDestination.devices,
        label: 'Devices',
        icon: LucideIcons.laptop,
      ),
      AppNavigationDestination(
        destination: AppDestination.settings,
        label: 'Settings',
        icon: LucideIcons.settings,
      ),
    ];

extension AppDestinationNavigationData on AppDestination {
  AppNavigationDestination get navigationDestination {
    return appNavigationDestinations.singleWhere(
      (AppNavigationDestination item) => item.destination == this,
    );
  }
}
