import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_theme.dart';
import '../../../l10n/l10n.dart';

/// Asks before an irreversible change. Cancel holds the initial focus, so
/// Enter or Space right after the dialog opens keeps everything as it was.
/// Resolves to true only when the change was confirmed.
Future<bool> showDestructiveConfirmation(
  BuildContext context, {
  required IconData icon,
  required String title,
  required String message,
  required String confirmLabel,
}) async {
  final confirmed = await AppOverlays.showDialog<bool>(
    context,
    builder: (_) => _ConfirmationDialog(
      icon: icon,
      title: title,
      message: message,
      confirmLabel: confirmLabel,
    ),
  );
  return confirmed ?? false;
}

class _ConfirmationDialog extends StatefulWidget {
  const _ConfirmationDialog({
    required this.icon,
    required this.title,
    required this.message,
    required this.confirmLabel,
  });

  final IconData icon;
  final String title;
  final String message;
  final String confirmLabel;

  @override
  State<_ConfirmationDialog> createState() => _ConfirmationDialogState();
}

class _ConfirmationDialogState extends State<_ConfirmationDialog> {
  final _cancelFocus = FocusNode(debugLabel: 'confirmation-cancel');

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _cancelFocus.requestFocus();
    });
  }

  @override
  void dispose() {
    _cancelFocus.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    void close(bool confirmed) => Navigator.of(context).pop(confirmed);
    return CallbackShortcuts(
      bindings: {
        const SingleActivator(LogicalKeyboardKey.escape): () => close(false),
      },
      child: Semantics(
        scopesRoute: true,
        namesRoute: true,
        explicitChildNodes: true,
        label: widget.title,
        child: AppOverlays.alertDialog(
          icon: widget.icon,
          title: Text(widget.title),
          content: Text(widget.message),
          actions: [
            Button.ghost(
              focusNode: _cancelFocus,
              alignment: Alignment.center,
              style: AppTheme.dialogButtonStyle(const ButtonStyle.ghost()),
              onPressed: () => close(false),
              child: Text(context.l10n.commonCancel),
            ),
            Button(
              alignment: Alignment.center,
              style: AppTheme.dialogButtonStyle(
                const ButtonStyle.destructive(),
              ),
              onPressed: () => close(true),
              child: Text(widget.confirmLabel),
            ),
          ],
        ),
      ),
    );
  }
}
