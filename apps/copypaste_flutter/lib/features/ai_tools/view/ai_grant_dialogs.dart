import 'dart:async';

import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../../../shared/state_view.dart';
import '../../../shared/system_date_time.dart';
import '../controller/ai_tools_controller.dart';
import '../models/ai_tools_models.dart';
import 'ai_scope_fields.dart';

/// Edits a connection's name, scopes and history window. A connection of
/// another desktop changes there once that desktop syncs.
Future<void> showAiGrantEditDialog(
  BuildContext context, {
  required AiToolsController controller,
  required AiToolGrant grant,
}) => AppOverlays.showDialog<void>(
  context,
  builder: (context) =>
      _AiGrantEditDialog(controller: controller, grant: grant),
);

/// The access log of a connection owned by this desktop.
Future<void> showAiGrantActivityDialog(
  BuildContext context, {
  required AiToolsController controller,
  required AiToolGrant grant,
}) => AppOverlays.showDialog<void>(
  context,
  builder: (context) =>
      _AiGrantActivityDialog(controller: controller, grant: grant),
);

class _AiGrantEditDialog extends StatefulWidget {
  const _AiGrantEditDialog({required this.controller, required this.grant});

  final AiToolsController controller;
  final AiToolGrant grant;

  @override
  State<_AiGrantEditDialog> createState() => _AiGrantEditDialogState();
}

class _AiGrantEditDialogState extends State<_AiGrantEditDialog> {
  late final _name = TextEditingController(text: widget.grant.name);
  late Set<AiToolScope> _scopes = {...widget.grant.scopes};
  late AiToolLookback _lookback = widget.grant.lookback;

  AiToolsController get _controller => widget.controller;

  @override
  void dispose() {
    _name.dispose();
    super.dispose();
  }

  bool get _canSave =>
      !_controller.busy && _scopes.isNotEmpty && _name.text.trim().isNotEmpty;

  Future<void> _save() async {
    if (!_canSave) return;
    final saved = await _controller.updateGrant(
      widget.grant,
      name: _name.text.trim(),
      scopes: _scopes,
      lookback: _lookback,
    );
    if (saved && mounted) Navigator.of(context).pop();
  }

  @override
  Widget build(BuildContext context) => ListenableBuilder(
    listenable: _controller,
    builder: (context, _) {
      final l10n = context.l10n;
      final busy = _controller.busy;
      return AppOverlays.alertDialog(
        icon: LucideIcons.pencil,
        title: Text(l10n.aiToolsEditTitle),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(l10n.aiToolsNameLabel).small(),
            const Gap(AppSpacing.xs),
            TextField(
              key: const ValueKey('ai-grant-name'),
              controller: _name,
              enabled: !busy,
              decoration: AppOverlays.dialogFieldDecoration(context),
              onChanged: (_) => setState(() {}),
            ),
            const Gap(AppSpacing.md),
            AiScopeFields(
              scopes: _scopes,
              lookback: _lookback,
              enabled: !busy,
              onScopesChanged: (scopes) => setState(() => _scopes = scopes),
              onLookbackChanged: (lookback) =>
                  setState(() => _lookback = lookback),
            ),
            if (_controller.actionError case final error?) ...[
              const Gap(AppSpacing.md),
              Alert.destructive(
                leading: const AppIconFrame(
                  child: Icon(LucideIcons.circleAlert),
                ),
                title: Text(l10n.appActionFailed),
                content: Text(error),
              ),
            ],
          ],
        ),
        actions: [
          Button.ghost(
            alignment: Alignment.center,
            style: AppTheme.dialogButtonStyle(const ButtonStyle.ghost()),
            onPressed: busy ? null : () => Navigator.of(context).maybePop(),
            child: Text(l10n.commonCancel),
          ),
          Button.primary(
            alignment: Alignment.center,
            style: AppTheme.dialogButtonStyle(const ButtonStyle.primary()),
            onPressed: _canSave ? () => unawaited(_save()) : null,
            child: Text(l10n.commonSave),
          ),
        ],
      );
    },
  );
}

class _AiGrantActivityDialog extends StatefulWidget {
  const _AiGrantActivityDialog({required this.controller, required this.grant});

  final AiToolsController controller;
  final AiToolGrant grant;

  @override
  State<_AiGrantActivityDialog> createState() => _AiGrantActivityDialogState();
}

class _AiGrantActivityDialogState extends State<_AiGrantActivityDialog> {
  late Future<List<AiAccessEntry>> _entries = _load();

  Future<List<AiAccessEntry>> _load() =>
      widget.controller.accessLog(widget.grant);

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return AppOverlays.alertDialog(
      icon: LucideIcons.history,
      title: Text(l10n.aiToolsActivityTitle(widget.grant.name)),
      content: FutureBuilder<List<AiAccessEntry>>(
        future: _entries,
        builder: (context, snapshot) {
          if (snapshot.hasError) {
            return StateView.error(
              title: l10n.aiToolsActivityFailed,
              actionLabel: l10n.commonTryAgain,
              onAction: () => setState(() => _entries = _load()),
            );
          }
          final entries = snapshot.data;
          if (entries == null) return const StateView.loading();
          if (entries.isEmpty) {
            return StateView.empty(
              compact: true,
              icon: LucideIcons.history,
              title: l10n.aiToolsActivityEmpty,
            );
          }
          return Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              for (final entry in entries)
                Padding(
                  padding: const EdgeInsets.symmetric(vertical: AppSpacing.xxs),
                  child: Row(
                    children: [
                      Expanded(
                        child: Text(
                          l10n.aiToolsActivityEntry(
                            entry.tool,
                            entry.resultCount,
                          ),
                        ).small(),
                      ),
                      const Gap(AppSpacing.sm),
                      Text(
                        [
                          formatSystemDateTime(context, entry.at),
                          ?entry.error,
                        ].join(' · '),
                      ).xSmall().muted(),
                    ],
                  ),
                ),
            ],
          );
        },
      ),
      actions: [
        Button.primary(
          alignment: Alignment.center,
          style: AppTheme.dialogButtonStyle(const ButtonStyle.primary()),
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.commonClose),
        ),
      ],
    );
  }
}
