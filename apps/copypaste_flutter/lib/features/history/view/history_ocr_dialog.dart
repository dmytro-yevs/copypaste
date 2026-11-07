import 'package:copypaste_flutter/app/theme/app_icon_frame.dart';

import 'dart:async';

import 'history_selectable_text.dart';

import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_toast.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../shared/state_view.dart';
import '../controller/history_ocr_controller.dart';
import '../models/history_models.dart';

Future<void> showHistoryOcrDialog(
  BuildContext context, {
  required HistoryOcrController controller,
  required HistoryClip clip,
}) async {
  if (!controller.canRun) return;
  unawaited(controller.recognize(clip));
  await AppOverlays.showDialog<void>(
    context,
    builder: (dialogContext) => AnimatedBuilder(
      animation: controller,
      builder: (context, _) {
        final text = controller.text;
        final content = controller.busy
            ? StateView.loading(message: context.l10n.historyOcrRecognizing)
            : controller.errorMessage != null
            ? StateView.error(
                title: context.l10n.historyOcrFailedTitle,
                message: controller.errorMessage,
                actionLabel: controller.canRun
                    ? context.l10n.commonTryAgain
                    : null,
                onAction: controller.canRun
                    ? () => unawaited(controller.recognize(clip))
                    : null,
              )
            : text == null || text.trim().isEmpty
            ? StateView.empty(title: context.l10n.historyOcrNoText)
            : SingleChildScrollView(
                child: HistorySelectableText(
                  text,
                  onCopy: controller.copySelection,
                ),
              );
        return AppOverlays.alertDialog(
          icon: LucideIcons.scanText,
          title: Text(context.l10n.historyOcrTitle),
          content: SizedBox(
            key: const ValueKey('history-ocr-content'),
            width: AppOverlaySize.dialogMaxWidth,
            height:
                MediaQuery.sizeOf(context).height *
                AppOverlaySize.dialogContentHeightFactor,
            child: content,
          ),
          actions: [
            Button.secondary(
              style: AppTheme.dialogButtonStyle(const ButtonStyle.secondary()),
              onPressed: () => Navigator.pop(dialogContext),
              child: Text(context.l10n.commonClose),
            ),
            Button.primary(
              style: AppTheme.dialogButtonStyle(const ButtonStyle.primary()),
              key: const ValueKey('history-ocr-copy'),
              onPressed: controller.busy || text == null || text.trim().isEmpty
                  ? null
                  : () async {
                      final copied = await controller.copyText();
                      if (!context.mounted) return;
                      AppToast.show(
                        context,
                        title: copied
                            ? context.l10n.historyCopiedTitle
                            : context.l10n.historyCopyFailedTitle,
                        message: copied
                            ? context.l10n.historyOcrTextCopied
                            : context.l10n.historyOcrCopyError,
                        tone: copied
                            ? AppToastTone.success
                            : AppToastTone.error,
                      );
                    },
              leading: const AppIconFrame(child: Icon(LucideIcons.copy)),
              child: Text(context.l10n.commonCopy),
            ),
          ],
        );
      },
    ),
  );
}
