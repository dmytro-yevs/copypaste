import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../shared/state_view.dart';
import '../../settings/controller/settings_navigation_state.dart';

/// Android quick access settings; a placeholder until the feature provides its controls.
class QuickAccessSettingsView extends StatelessWidget {
  const QuickAccessSettingsView({super.key});

  static const _section = SettingsSectionId.quickAccess;

  @override
  Widget build(BuildContext context) => StateView.empty(
    compact: true,
    icon: _section.icon,
    title: _section.label,
    message: _section.description,
  );
}
