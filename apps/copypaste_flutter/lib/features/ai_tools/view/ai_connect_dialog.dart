import 'dart:async';

import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_toast.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../controller/ai_tools_controller.dart';
import '../models/ai_tools_models.dart';
import '../presentation/ai_tools_labels.dart';
import 'ai_scope_fields.dart';

Future<void> showAiConnectDialog(
  BuildContext context, {
  required AiToolsController controller,
}) => AppOverlays.showDialog<void>(
  context,
  barrierDismissible: false,
  builder: (context) => AiConnectDialog(controller: controller),
);

/// Connect AI tool: pick a client, scopes and history window, then copy the
/// configuration that embeds the new credential. The credential is shown
/// only here and is copied without passing through clipboard history.
class AiConnectDialog extends StatefulWidget {
  const AiConnectDialog({super.key, required this.controller});

  final AiToolsController controller;

  @override
  State<AiConnectDialog> createState() => _AiConnectDialogState();
}

class _AiConnectDialogState extends State<AiConnectDialog> {
  final _name = TextEditingController();
  AiToolClient _client = AiToolClient.claudeDesktop;
  bool _nameEdited = false;
  Set<AiToolScope> _scopes = {...defaultAiToolScopes};
  late AiToolLookback _lookback = widget.controller.policy.defaultLookback;
  AiToolGrantCreated? _created;

  AiToolsController get _controller => widget.controller;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (!_nameEdited) _name.text = context.l10n.aiToolsClient(_client);
  }

  @override
  void dispose() {
    _name.dispose();
    super.dispose();
  }

  bool get _canCreate =>
      !_controller.busy && _scopes.isNotEmpty && _name.text.trim().isNotEmpty;

  Future<void> _create() async {
    if (!_canCreate) return;
    final created = await _controller.createGrant(
      name: _name.text.trim(),
      client: _client,
      scopes: _scopes,
      lookback: _lookback,
    );
    if (created != null && mounted) setState(() => _created = created);
  }

  Future<void> _copy(AiClientConfig config) async {
    final copied = await _controller.copySecret(config.text);
    if (!mounted) return;
    final l10n = context.l10n;
    AppToast.show(
      context,
      title: copied ? l10n.aiToolsCopied : l10n.appActionFailed,
      message: copied ? l10n.aiToolsCopiedMessage : l10n.aiToolsCopyFailed,
      tone: copied ? AppToastTone.success : AppToastTone.error,
    );
  }

  @override
  Widget build(BuildContext context) => ListenableBuilder(
    listenable: _controller,
    builder: (context, _) {
      final l10n = context.l10n;
      final created = _created;
      return AppOverlays.alertDialog(
        icon: LucideIcons.plug,
        title: Text(l10n.aiToolsConnect),
        content: created == null ? _form(context) : _result(context, created),
        actions: created == null
            ? [
                Button.ghost(
                  alignment: Alignment.center,
                  style: AppTheme.dialogButtonStyle(const ButtonStyle.ghost()),
                  onPressed: _controller.busy
                      ? null
                      : () => Navigator.of(context).maybePop(),
                  child: Text(l10n.commonCancel),
                ),
                Button.primary(
                  key: const ValueKey('ai-connect-create'),
                  alignment: Alignment.center,
                  style: AppTheme.dialogButtonStyle(
                    const ButtonStyle.primary(),
                  ),
                  onPressed: _canCreate ? () => unawaited(_create()) : null,
                  child: Text(l10n.aiToolsCreate),
                ),
              ]
            : [
                Button.primary(
                  key: const ValueKey('ai-connect-done'),
                  alignment: Alignment.center,
                  style: AppTheme.dialogButtonStyle(
                    const ButtonStyle.primary(),
                  ),
                  onPressed: () => Navigator.of(context).pop(),
                  child: Text(l10n.commonDone),
                ),
              ],
      );
    },
  );

  Widget _form(BuildContext context) {
    final l10n = context.l10n;
    final busy = _controller.busy;
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(l10n.aiToolsClientLabel).small(),
        const Gap(AppSpacing.xs),
        Select<AiToolClient>(
          key: const ValueKey('ai-connect-client'),
          enabled: !busy,
          value: _client,
          onChanged: (client) {
            if (client == null) return;
            setState(() {
              _client = client;
              if (!_nameEdited) _name.text = l10n.aiToolsClient(client);
            });
          },
          itemBuilder: (context, client) => Text(l10n.aiToolsClient(client)),
          popup: SelectPopup<AiToolClient>(
            items: SelectItemList(
              children: [
                for (final client in AiToolClient.values)
                  SelectItemButton<AiToolClient>(
                    value: client,
                    child: Text(l10n.aiToolsClient(client)),
                  ),
              ],
            ),
          ).call,
        ),
        const Gap(AppSpacing.md),
        Text(l10n.aiToolsNameLabel).small(),
        const Gap(AppSpacing.xs),
        TextField(
          key: const ValueKey('ai-connect-name'),
          controller: _name,
          enabled: !busy,
          decoration: AppOverlays.dialogFieldDecoration(context),
          onChanged: (_) => setState(() => _nameEdited = true),
        ),
        const Gap(AppSpacing.md),
        AiScopeFields(
          scopes: _scopes,
          lookback: _lookback,
          enabled: !busy,
          onScopesChanged: (scopes) => setState(() => _scopes = scopes),
          onLookbackChanged: (lookback) => setState(() => _lookback = lookback),
        ),
        if (_controller.actionError case final error?) ...[
          const Gap(AppSpacing.md),
          Alert.destructive(
            leading: const AppIconFrame(child: Icon(LucideIcons.circleAlert)),
            title: Text(l10n.appActionFailed),
            content: Text(error),
          ),
        ],
      ],
    );
  }

  Widget _result(BuildContext context, AiToolGrantCreated created) {
    final l10n = context.l10n;
    final client = _client.brandName;
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          client == null
              ? l10n.aiToolsConfigHintOther
              : l10n.aiToolsConfigHint(client),
        ),
        for (final config in created.configs) ...[
          const Gap(AppSpacing.md),
          CodeSnippet(
            key: ValueKey('ai-connect-config-${config.client}'),
            code: Text(config.text),
            actions: [
              Tooltip(
                tooltip: (_) => TooltipContainer(child: Text(l10n.commonCopy)),
                child: Button.ghost(
                  key: const ValueKey('ai-connect-copy'),
                  style: AppTheme.controlButtonStyle(
                    const ButtonStyle.ghostIcon(),
                  ),
                  onPressed: () => unawaited(_copy(config)),
                  child: Semantics(
                    label: l10n.commonCopy,
                    child: const AppIconFrame(child: Icon(LucideIcons.copy)),
                  ),
                ),
              ),
            ],
          ),
        ],
        const Gap(AppSpacing.md),
        Text(
          client == null
              ? l10n.aiToolsRestartNoteOther
              : l10n.aiToolsRestartNote(client),
        ),
      ],
    );
  }
}
