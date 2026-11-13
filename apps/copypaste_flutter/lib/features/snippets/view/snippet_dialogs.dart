import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_theme.dart';
import '../../../l10n/l10n.dart';
import '../../../platform/security/device_authenticator.dart';
import '../controller/concealed_snippet_access.dart';
import '../controller/snippets_controller.dart';
import '../models/snippet_models.dart';

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

/// A yes/no question; true only when [confirmLabel] was chosen.
Future<bool> showSnippetQuestion(
  BuildContext context, {
  required IconData icon,
  required String title,
  required String message,
  required String confirmLabel,
  String? declineLabel,
}) async {
  final answer = await AppOverlays.showDialog<bool>(
    context,
    builder: (context) => AppOverlays.alertDialog(
      icon: icon,
      title: Text(title),
      content: Text(message),
      actions: [
        Button.ghost(
          alignment: Alignment.center,
          style: AppTheme.dialogButtonStyle(const ButtonStyle.ghost()),
          onPressed: () => Navigator.pop(context, false),
          child: Text(declineLabel ?? context.l10n.commonCancel),
        ),
        Button.primary(
          alignment: Alignment.center,
          style: AppTheme.dialogButtonStyle(const ButtonStyle.primary()),
          onPressed: () => Navigator.pop(context, true),
          child: Text(confirmLabel),
        ),
      ],
    ),
  );
  return answer ?? false;
}

/// Verifies the device owner before a concealed snippet is used or shown
/// while `snippets.require_auth` is on.
Future<bool> ensureSnippetAccess(
  BuildContext context,
  SnippetsController controller,
  SnippetSummary snippet,
) async =>
    !controller.needsUnlock(snippet) ||
    await ensureOwnerAccess(context, controller);

/// Verifies the device owner while `snippets.require_auth` is on, such as
/// before that protection is turned off. Systems without device
/// authentication confirm in the app instead.
Future<bool> ensureOwnerAccess(
  BuildContext context,
  SnippetsController controller,
) async {
  if (!controller.ownerLocked) return true;
  final l10n = context.l10n;
  final access = controller.access;
  final result = access == null
      ? ConcealedAccess.confirmInApp
      : await access.request(
          deviceAuthReason(l10n, DeviceAuthPurpose.useConcealedSnippet),
        );
  if (result == ConcealedAccess.granted) return true;
  if (result == ConcealedAccess.denied || !context.mounted) return false;
  final confirmed = await showSnippetQuestion(
    context,
    icon: LucideIcons.lock,
    title: l10n.snippetsConcealedConfirmTitle,
    message: l10n.snippetsConcealedConfirmMessage,
    confirmLabel: l10n.snippetsConcealedConfirmAction,
  );
  if (confirmed) access?.grant();
  return confirmed;
}
