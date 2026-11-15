import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../app/theme/app_overlays.dart';
import '../../app/theme/app_theme.dart';
import '../../l10n/l10n.dart';

/// A yes/no question; true only when [confirmLabel] was chosen.
Future<bool> showQuestionDialog(
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

/// Asks the user to confirm it is them where the device cannot verify its
/// owner.
Future<bool> confirmOwnerInApp(BuildContext context) {
  final l10n = context.l10n;
  return showQuestionDialog(
    context,
    icon: LucideIcons.lock,
    title: l10n.snippetsConcealedConfirmTitle,
    message: l10n.snippetsConcealedConfirmMessage,
    confirmLabel: l10n.snippetsConcealedConfirmAction,
  );
}
