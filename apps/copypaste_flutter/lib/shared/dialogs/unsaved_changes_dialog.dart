import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../app/theme/app_overlays.dart';
import '../../app/theme/app_theme.dart';
import '../../l10n/l10n.dart';

enum UnsavedChoice { save, discard, cancel }

/// Save / Discard / Cancel before an editor with unsaved changes closes.
/// Dismissing the dialog cancels.
Future<UnsavedChoice> showUnsavedChangesDialog(BuildContext context) async {
  final choice = await AppOverlays.showDialog<UnsavedChoice>(
    context,
    builder: (context) {
      final l10n = context.l10n;
      void close(UnsavedChoice choice) => Navigator.pop(context, choice);
      return CallbackShortcuts(
        bindings: {
          const SingleActivator(LogicalKeyboardKey.escape): () =>
              close(UnsavedChoice.cancel),
        },
        child: AppOverlays.alertDialog(
          icon: LucideIcons.circleAlert,
          title: Text(l10n.snippetsUnsavedTitle),
          content: Text(l10n.snippetsUnsavedMessage),
          actions: [
            Button.ghost(
              key: const ValueKey('unsaved-cancel'),
              alignment: Alignment.center,
              style: AppTheme.dialogButtonStyle(const ButtonStyle.ghost()),
              onPressed: () => close(UnsavedChoice.cancel),
              child: Text(l10n.commonCancel),
            ),
            Button.secondary(
              key: const ValueKey('unsaved-discard'),
              alignment: Alignment.center,
              style: AppTheme.dialogButtonStyle(const ButtonStyle.secondary()),
              onPressed: () => close(UnsavedChoice.discard),
              child: Text(l10n.snippetsDiscard),
            ),
            Button.primary(
              key: const ValueKey('unsaved-save'),
              alignment: Alignment.center,
              style: AppTheme.dialogButtonStyle(const ButtonStyle.primary()),
              onPressed: () => close(UnsavedChoice.save),
              child: Text(l10n.commonSave),
            ),
          ],
        ),
      );
    },
  );
  return choice ?? UnsavedChoice.cancel;
}
