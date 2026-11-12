import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../l10n/l10n.dart';
import '../../../shared/state_view.dart';

/// Paste Queue settings, embedded in Quick Paste on desktop and in Quick
/// access on Android; a placeholder until the feature provides its controls.
class PasteQueueSettingsView extends StatelessWidget {
  const PasteQueueSettingsView({super.key});

  @override
  Widget build(BuildContext context) => StateView.empty(
    compact: true,
    icon: LucideIcons.listOrdered,
    title: context.l10n.pasteQueueSettingsTitle,
    message: context.l10n.pasteQueueSettingsSummary,
  );
}
