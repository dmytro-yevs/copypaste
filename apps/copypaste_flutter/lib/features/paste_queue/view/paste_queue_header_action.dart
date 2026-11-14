import 'dart:async';

import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/shell/header_action_content.dart';
import '../../../app/shell/header_fit.dart';
import '../../../app/theme/app_filter_indicator.dart';
import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_motion.dart';
import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_theme.dart';
import '../../../l10n/l10n.dart';
import '../controller/paste_queue_controller.dart';
import '../controller/paste_queue_shortcuts_controller.dart';
import 'paste_queue_drawer.dart';

/// The queue's header control: the pending count, the collecting state, and
/// the queue drawer. It is left out while the queue is turned off.
class PasteQueueHeaderAction extends StatefulWidget {
  const PasteQueueHeaderAction({
    super.key,
    required this.controller,
    this.shortcuts,
    this.onOpenSettings,
    this.onDrawerVisibilityChanged,
  });

  final PasteQueueController controller;
  final PasteQueueShortcutsController? shortcuts;
  final VoidCallback? onOpenSettings;
  final ValueChanged<bool>? onDrawerVisibilityChanged;

  @override
  State<PasteQueueHeaderAction> createState() => PasteQueueHeaderActionState();
}

class PasteQueueHeaderActionState extends State<PasteQueueHeaderAction> {
  OverlayCompleter<void>? _drawer;
  bool _opening = false;

  @override
  void dispose() {
    if (_drawer?.isCompleted == false) _drawer!.remove();
    super.dispose();
  }

  /// Opens the drawer, for example from the tray's queue item.
  Future<void> open() async {
    if (_opening || !mounted) return;
    _opening = true;
    widget.onDrawerVisibilityChanged?.call(true);
    try {
      final drawer = _drawer = showOverlay<void>(
        context,
        AppOverlays.drawerConfiguration(context),
        builder: (context) => PasteQueueDrawer(
          controller: widget.controller,
          shortcuts: widget.shortcuts,
          onOpenSettings: widget.onOpenSettings,
        ),
      );
      await drawer.future;
    } finally {
      _drawer = null;
      _opening = false;
      widget.onDrawerVisibilityChanged?.call(false);
    }
  }

  @override
  Widget build(BuildContext context) => ListenableBuilder(
    listenable: widget.controller,
    builder: (context, _) {
      final controller = widget.controller;
      if (!controller.enabled) return const SizedBox.shrink();
      final l10n = context.l10n;
      final count = controller.pendingCount;
      final collecting = controller.collecting;
      final summary = collecting
          ? l10n.pasteQueueSummaryCollecting(count)
          : l10n.pasteQueueSummary(count);
      return Tooltip(
        showDuration: AppMotion.resolve(context, AppMotion.standard),
        tooltip: (context) => TooltipContainer(
          child: Text('${l10n.pasteQueueSettingsTitle} · $summary'),
        ),
        child: Semantics(
          label: l10n.pasteQueueSettingsTitle,
          value: summary,
          button: true,
          child: ExcludeSemantics(
            child: Button.secondary(
              key: const ValueKey('paste-queue-open'),
              style: AppTheme.headerActionButtonStyle(
                const ButtonStyle.secondaryIcon(),
              ),
              onPressed: () => unawaited(open()),
              child: HeaderActionContent(
                icon: AppFilterIndicator.count(
                  count: count,
                  child: AppIconFrame(
                    child: Icon(
                      collecting
                          ? LucideIcons.listPlus
                          : LucideIcons.listOrdered,
                    ),
                  ),
                ),
                label: collecting ? l10n.pasteQueueCollectingLabel : null,
                labelUntil: HeaderStage.bottomNavigation,
              ),
            ),
          ),
        ),
      );
    },
  );
}
