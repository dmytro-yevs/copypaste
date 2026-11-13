import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../models/ai_tools_models.dart';
import 'ai_scope_fields.dart';

/// Asks whether an AI tool that has no connection may access CopyPaste.
/// Escape, like Deny, refuses; the dialog cannot be dismissed otherwise.
Future<AiApprovalDecision?> showAiApprovalDialog(
  BuildContext context, {
  Key? key,
  required AiApprovalRequest request,
  required AiToolLookback defaultLookback,
}) => AppOverlays.showDialog<AiApprovalDecision>(
  context,
  barrierDismissible: false,
  builder: (context) => AiApprovalDialog(
    key: key,
    request: request,
    defaultLookback: defaultLookback,
  ),
);

/// The first-use prompt: Deny holds the initial focus, and scopes and the
/// history window are chosen in the prompt itself.
class AiApprovalDialog extends StatefulWidget {
  const AiApprovalDialog({
    super.key,
    required this.request,
    required this.defaultLookback,
  });

  final AiApprovalRequest request;
  final AiToolLookback defaultLookback;

  @override
  State<AiApprovalDialog> createState() => _AiApprovalDialogState();
}

class _AiApprovalDialogState extends State<AiApprovalDialog> {
  late Set<AiToolScope> _scopes = {...defaultAiToolScopes};
  late AiToolLookback _lookback = widget.defaultLookback;

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final request = widget.request;
    return AiDecisionFrame(
      icon: LucideIcons.bot,
      title: l10n.aiToolsApprovalTitle(request.clientName),
      onAllow: _scopes.isEmpty
          ? null
          : () => Navigator.pop(
              context,
              AiApprovalDecision.allow(scopes: _scopes, lookback: _lookback),
            ),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text(l10n.aiToolsReportedName(request.clientName)),
          if (request.executable case final executable?) ...[
            const Gap(AppSpacing.xs),
            Text(l10n.aiToolsReportedExecutable(executable)),
          ],
          const Gap(AppSpacing.sm),
          Text(l10n.aiToolsApprovalNote),
          const Gap(AppSpacing.lg),
          AiScopeFields(
            scopes: _scopes,
            lookback: _lookback,
            onScopesChanged: (scopes) => setState(() => _scopes = scopes),
            onLookbackChanged: (lookback) =>
                setState(() => _lookback = lookback),
          ),
        ],
      ),
    );
  }
}

/// The dialog frame both AI tool prompts share: Deny takes the initial
/// focus and Escape denies; Allow needs explicit activation.
class AiDecisionFrame extends StatefulWidget {
  const AiDecisionFrame({
    super.key,
    required this.icon,
    required this.title,
    required this.content,
    required this.onAllow,
  });

  final IconData icon;
  final String title;
  final Widget content;
  final VoidCallback? onAllow;

  @override
  State<AiDecisionFrame> createState() => _AiDecisionFrameState();
}

class _AiDecisionFrameState extends State<AiDecisionFrame> {
  final _denyFocus = FocusNode(debugLabel: 'ai-deny');

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _denyFocus.requestFocus();
    });
  }

  @override
  void dispose() {
    _denyFocus.dispose();
    super.dispose();
  }

  void _deny() => Navigator.pop(context, const AiApprovalDecision.deny());

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return PopScope(
      canPop: false,
      child: CallbackShortcuts(
        bindings: {const SingleActivator(LogicalKeyboardKey.escape): _deny},
        child: FocusScope(
          child: AppOverlays.alertDialog(
            icon: widget.icon,
            title: Semantics(
              header: true,
              liveRegion: true,
              child: Text(widget.title),
            ),
            content: widget.content,
            actions: [
              Button.secondary(
                key: const ValueKey('ai-decision-deny'),
                focusNode: _denyFocus,
                alignment: Alignment.center,
                style: AppTheme.dialogButtonStyle(
                  const ButtonStyle.secondary(),
                ),
                onPressed: _deny,
                child: Text(l10n.aiToolsDeny),
              ),
              Button.primary(
                key: const ValueKey('ai-decision-allow'),
                alignment: Alignment.center,
                style: AppTheme.dialogButtonStyle(const ButtonStyle.primary()),
                onPressed: widget.onAllow,
                child: Text(l10n.aiToolsAllow),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
