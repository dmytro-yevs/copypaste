import 'dart:async';

import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_toast.dart';
import '../../../l10n/l10n.dart';
import '../controller/ai_approval_controller.dart';
import '../models/ai_tools_models.dart';
import 'ai_approval_dialog.dart';
import 'ai_volume_dialog.dart';

/// Shows the prompt for [AiApprovalController.current] above [child], one
/// at a time, and closes it when the request resolves elsewhere or times
/// out. It must sit below the app's navigator and overlay.
class AiApprovalPresenter extends StatefulWidget {
  const AiApprovalPresenter({
    super.key,
    required this.controller,
    required this.child,
  });

  final AiApprovalController controller;
  final Widget child;

  @override
  State<AiApprovalPresenter> createState() => _AiApprovalPresenterState();
}

class _AiApprovalPresenterState extends State<AiApprovalPresenter> {
  String? _shown;
  GlobalKey? _dialog;
  bool _closing = false;

  @override
  void initState() {
    super.initState();
    widget.controller.addListener(_changed);
    WidgetsBinding.instance.addPostFrameCallback((_) => _changed());
  }

  @override
  void didUpdateWidget(AiApprovalPresenter oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.controller != widget.controller) {
      oldWidget.controller.removeListener(_changed);
      widget.controller.addListener(_changed);
    }
  }

  @override
  void dispose() {
    widget.controller.removeListener(_changed);
    super.dispose();
  }

  void _changed() {
    if (!mounted) return;
    final shown = _shown;
    if (shown != null) {
      final stillPending = widget.controller.pending.any(
        (request) => request.requestId == shown,
      );
      final dialog = _dialog?.currentContext;
      if (!stillPending && !_closing && dialog != null) {
        _closing = true;
        Navigator.of(dialog).pop();
      }
      return;
    }
    final request = widget.controller.current;
    if (request != null) unawaited(_present(request));
  }

  Future<void> _present(AiApprovalRequest request) async {
    _shown = request.requestId;
    _closing = false;
    final key = _dialog = GlobalKey(debugLabel: 'ai-approval');
    final decision = await switch (request.kind) {
      AiApprovalKind.access => showAiApprovalDialog(
        context,
        key: key,
        request: request,
        defaultLookback: widget.controller.defaultLookback,
      ),
      AiApprovalKind.volume => showAiVolumeDialog(
        context,
        key: key,
        request: request,
      ),
    };
    final closedForUs = _closing;
    _shown = null;
    _dialog = null;
    _closing = false;
    if (!mounted) return;
    if (!closedForUs) {
      final sent = await widget.controller.resolve(
        request,
        decision ?? const AiApprovalDecision.deny(),
      );
      if (!sent && mounted) {
        AppToast.show(
          context,
          title: context.l10n.appActionFailed,
          message: context.l10n.aiToolsDecisionFailed,
          tone: AppToastTone.error,
        );
      }
    }
    if (mounted) _changed();
  }

  @override
  Widget build(BuildContext context) => widget.child;
}
