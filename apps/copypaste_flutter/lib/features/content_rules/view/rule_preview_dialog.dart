import 'dart:async';

import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../../../shared/system_date_time.dart';
import '../../settings/view/confirmation_dialog.dart';
import '../controller/rule_preview_controller.dart';
import '../models/content_rule_models.dart';
import '../repository/content_rule_repository.dart';

/// Previews a draft ([definitionJson]) or saved rule ([ruleId]) against the
/// latest clips, then applies it on confirmation. Closing releases the plan.
Future<void> showRulePreviewDialog(
  BuildContext context, {
  required ContentRuleRepository repository,
  String? definitionJson,
  String? ruleId,
  bool fullSet = false,
}) => AppOverlays.showDialog<void>(
  context,
  barrierDismissible: false,
  builder: (context) => RulePreviewDialog(
    controller: RulePreviewController(
      repository: repository,
      definitionJson: definitionJson,
      ruleId: ruleId,
      fullSet: fullSet,
    ),
  ),
);

/// Count, list and Apply confirmation of a rule preview (content-rules 5.4).
/// It owns [controller] and disposes it.
class RulePreviewDialog extends StatefulWidget {
  const RulePreviewDialog({super.key, required this.controller});

  final RulePreviewController controller;

  @override
  State<RulePreviewDialog> createState() => _RulePreviewDialogState();
}

class _RulePreviewDialogState extends State<RulePreviewDialog> {
  /// Rows shown; the counts cover every matched clip.
  static const _shownItems = 100;

  RulePreviewController get _controller => widget.controller;

  @override
  void initState() {
    super.initState();
    unawaited(_controller.start());
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  void _close() {
    if (_controller.phase == RulePreviewPhase.applying) return;
    unawaited(_controller.close());
    Navigator.of(context).pop();
  }

  Future<void> _apply() async {
    final preview = _controller.preview;
    if (preview == null) return;
    if (_controller.destructive) {
      final l10n = context.l10n;
      final confirmed = await showDestructiveConfirmation(
        context,
        icon: LucideIcons.trash2,
        title: l10n.rulesApplyDeleteTitle(preview.counts.deleted),
        message: l10n.rulesApplyDeleteMessage,
        confirmLabel: l10n.rulesApplyDeleteConfirm,
      );
      if (!confirmed || !mounted) return;
    }
    await _controller.apply();
  }

  @override
  Widget build(BuildContext context) => ListenableBuilder(
    listenable: _controller,
    builder: (context, _) {
      final l10n = context.l10n;
      final phase = _controller.phase;
      final running =
          phase == RulePreviewPhase.previewing ||
          phase == RulePreviewPhase.applying;
      return PopScope(
        canPop: phase != RulePreviewPhase.applying,
        onPopInvokedWithResult: (didPop, _) {
          if (didPop) unawaited(_controller.close());
        },
        child: CallbackShortcuts(
          bindings: {
            const SingleActivator(LogicalKeyboardKey.escape): running
                ? () => unawaited(_controller.cancel())
                : _close,
          },
          child: AppOverlays.alertDialog(
            icon: LucideIcons.history,
            title: Text(l10n.rulesPreviewTitle),
            content: KeyedSubtree(
              key: const ValueKey('rule-preview-dialog'),
              child: _content(context),
            ),
            actions: _actions(context),
          ),
        ),
      );
    },
  );

  List<Widget> _actions(BuildContext context) {
    final l10n = context.l10n;
    Widget button(
      String label,
      ButtonStyle style,
      VoidCallback? onPressed, {
      Key? key,
    }) => Button(
      key: key,
      alignment: Alignment.center,
      style: AppTheme.dialogButtonStyle(style),
      onPressed: onPressed,
      child: Text(label),
    );
    return switch (_controller.phase) {
      RulePreviewPhase.previewing || RulePreviewPhase.applying => [
        button(
          l10n.commonCancel,
          const ButtonStyle.secondary(),
          () => unawaited(_controller.cancel()),
          key: const ValueKey('rule-preview-cancel'),
        ),
      ],
      RulePreviewPhase.ready => [
        button(l10n.commonClose, const ButtonStyle.ghost(), _close),
        button(
          l10n.commonApply,
          _controller.destructive
              ? const ButtonStyle.destructive()
              : const ButtonStyle.primary(),
          _controller.canApply ? () => unawaited(_apply()) : null,
          key: const ValueKey('rule-preview-apply'),
        ),
      ],
      _ => [
        button(
          l10n.commonClose,
          const ButtonStyle.primary(),
          _close,
          key: const ValueKey('rule-preview-close'),
        ),
      ],
    };
  }

  Widget _content(BuildContext context) {
    final l10n = context.l10n;
    switch (_controller.phase) {
      case RulePreviewPhase.previewing:
        return Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(l10n.rulesPreviewRunning(RuleLimits.previewItems)),
            const Gap(AppSpacing.md),
            const LinearProgressIndicator(),
          ],
        );
      case RulePreviewPhase.applying:
        final progress = _controller.progress;
        final total = progress?.total ?? 0;
        return Semantics(
          liveRegion: true,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text(l10n.rulesApplyProgress(progress?.done ?? 0, total)),
              const Gap(AppSpacing.md),
              LinearProgressIndicator(
                value: total == 0 ? null : (progress!.done / total),
              ),
            ],
          ),
        );
      case RulePreviewPhase.failed:
        return Alert.destructive(
          leading: const AppIconFrame(child: Icon(LucideIcons.circleAlert)),
          title: Text(l10n.rulesPreviewFailed),
          content: Text(_controller.errorMessage ?? ''),
        );
      case RulePreviewPhase.cancelled:
        return Text(
          _controller.report == null
              ? l10n.rulesPreviewCancelled
              : l10n.rulesApplyCancelled,
        );
      case RulePreviewPhase.applied:
        final report = _controller.report!;
        return Semantics(
          liveRegion: true,
          child: Column(
            key: const ValueKey('rule-preview-report'),
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text(l10n.rulesApplyChanged(report.changed)),
              if (report.deleted > 0)
                Text(l10n.rulesApplyDeleted(report.deleted)),
              if (report.failed > 0) Text(l10n.rulesApplyFailed(report.failed)),
            ],
          ),
        );
      case RulePreviewPhase.ready:
        return _result(context, _controller.preview!);
    }
  }

  Widget _result(BuildContext context, RulePreview preview) {
    final l10n = context.l10n;
    final counts = preview.counts;
    final effects = [
      if (counts.deleted > 0) l10n.rulesPreviewDeleted(counts.deleted),
      if (counts.secret > 0) l10n.rulesPreviewSecret(counts.secret),
      if (counts.hiddenFromAi > 0) l10n.rulesPreviewHidden(counts.hiddenFromAi),
      if (counts.expiring > 0) l10n.rulesPreviewExpiring(counts.expiring),
      if (counts.tagged > 0) l10n.rulesPreviewTagged(counts.tagged),
    ];
    final shown = preview.items.take(_shownItems).toList();
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          counts.matched == 0
              ? l10n.rulesPreviewNone
              : l10n.rulesPreviewMatched(counts.matched),
          key: const ValueKey('rule-preview-count'),
        ).medium(),
        for (final effect in effects) Text(effect).small(),
        if (preview.truncated) ...[
          const Gap(AppSpacing.xs),
          Text(
            l10n.rulesPreviewTruncated(RuleLimits.previewItems),
          ).xSmall().muted(),
        ],
        if (counts.deleted > 0) ...[
          const Gap(AppSpacing.sm),
          Alert.destructive(
            leading: const AppIconFrame(child: Icon(LucideIcons.trash2)),
            content: Text(l10n.rulesApplyDeleteMessage),
          ),
        ],
        if (shown.isNotEmpty) ...[
          const Gap(AppSpacing.md),
          Card(
            theme: AppTheme.settingsGroupCardTheme,
            padding: const EdgeInsets.symmetric(vertical: AppSpacing.xs),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                for (final (index, item) in shown.indexed) ...[
                  if (index > 0) const Divider(),
                  _item(context, item),
                ],
              ],
            ),
          ),
          if (preview.items.length > shown.length) ...[
            const Gap(AppSpacing.xs),
            Text(
              l10n.rulesPreviewMore(preview.items.length - shown.length),
            ).xSmall().muted(),
          ],
        ],
        const Gap(AppSpacing.md),
        Text(l10n.rulesPreviewNote).xSmall().muted(),
      ],
    );
  }

  Widget _item(BuildContext context, RulePreviewItem item) {
    final l10n = context.l10n;
    final text = item.masked
        ? (item.sourceAppName == null
              ? l10n.rulesSecretItem
              : l10n.rulesSecretItemFrom(item.sourceAppName!))
        : item.preview.trim().split('\n').first;
    final details = [
      ?item.sourceAppName,
      formatSystemRecentTime(context, item.createdAt),
    ].join(' · ');
    return Padding(
      padding: const EdgeInsets.symmetric(
        horizontal: AppSpacing.md,
        vertical: AppSpacing.xs,
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              if (item.masked) ...[
                const AppIconFrame(
                  size: AppIconSize.sm,
                  child: Icon(LucideIcons.lock, size: AppIconSize.sm),
                ),
                const Gap(AppSpacing.xs),
              ],
              Expanded(
                child: Text(
                  text,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ).small(),
              ),
            ],
          ),
          Text(
            details,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
          ).xSmall().muted(),
        ],
      ),
    );
  }
}
