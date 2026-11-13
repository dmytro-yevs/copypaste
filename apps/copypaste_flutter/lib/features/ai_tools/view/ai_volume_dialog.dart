import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_overlays.dart';
import '../../../l10n/l10n.dart';
import '../models/ai_tools_models.dart';
import 'ai_approval_dialog.dart';

/// Asks whether an AI tool session that read a lot may read more.
Future<AiApprovalDecision?> showAiVolumeDialog(
  BuildContext context, {
  Key? key,
  required AiApprovalRequest request,
}) => AppOverlays.showDialog<AiApprovalDecision>(
  context,
  barrierDismissible: false,
  builder: (context) => AiVolumeDialog(key: key, request: request),
);

/// The read-volume confirmation, in the same frame as the approval prompt.
class AiVolumeDialog extends StatelessWidget {
  const AiVolumeDialog({super.key, required this.request});

  final AiApprovalRequest request;

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return AiDecisionFrame(
      icon: LucideIcons.gauge,
      title: l10n.aiToolsVolumeTitle,
      content: Text(l10n.aiToolsVolumeMessage(request.displayName)),
      onAllow: () => Navigator.pop(context, const AiApprovalDecision.allow()),
    );
  }
}
