import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../app/theme/app_overlays.dart';
import '../../app/theme/app_theme.dart';
import '../../app/theme/app_tokens.dart';
import '../../shared/state_view.dart';
import 'notifications_controller.dart';

/// Uses the same responsive drawer and control sizing as Downloads.
class NotificationsHeaderAction extends StatefulWidget {
  const NotificationsHeaderAction({
    super.key,
    required this.controller,
    this.onDrawerVisibilityChanged,
  });
  final NotificationsController controller;
  final ValueChanged<bool>? onDrawerVisibilityChanged;
  @override
  State<NotificationsHeaderAction> createState() =>
      _NotificationsHeaderActionState();
}

class _NotificationsHeaderActionState extends State<NotificationsHeaderAction> {
  OverlayCompleter<void>? _drawer;
  @override
  void dispose() {
    if (_drawer?.isCompleted == false) _drawer!.remove();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => ListenableBuilder(
    listenable: widget.controller,
    builder: (context, _) => Tooltip(
      tooltip: (_) => TooltipContainer(
        child: Text('Notifications · ${widget.controller.unread} unread'),
      ),
      child: Button.secondary(
        key: const ValueKey('notifications-open'),
        style: AppTheme.controlButtonStyle(const ButtonStyle.secondaryIcon()),
        onPressed: _open,
        child: Icon(
          widget.controller.unread > 0 ? LucideIcons.bellDot : LucideIcons.bell,
        ),
      ),
    ),
  );
  Future<void> _open() async {
    if (_drawer?.isCompleted == false) return;
    widget.onDrawerVisibilityChanged?.call(true);
    widget.controller.markAllRead();
    try {
      final drawer = _drawer = showOverlay<void>(
        context,
        AppOverlays.drawerConfiguration(context),
        builder: (context) => ConstrainedBox(
          constraints: AppOverlays.drawerContentConstraints(context),
          child: _NotificationsDrawer(controller: widget.controller),
        ),
      );
      await drawer.future;
    } finally {
      _drawer = null;
      widget.onDrawerVisibilityChanged?.call(false);
    }
  }
}

class _NotificationsDrawer extends StatelessWidget {
  const _NotificationsDrawer({required this.controller});
  final NotificationsController controller;
  @override
  Widget build(BuildContext context) => ListenableBuilder(
    listenable: controller,
    builder: (context, _) => Scaffold(
      headers: [
        AppBar(
          title: const Text('Notifications'),
          trailing: [
            Button.ghost(
              style: AppTheme.controlButtonStyle(const ButtonStyle.ghostIcon()),
              onPressed: () => closeDrawer(context),
              child: const Icon(LucideIcons.x),
            ),
          ],
        ),
        const Divider(),
      ],
      footers: [
        const Divider(),
        Padding(
          padding: const EdgeInsets.all(AppSpacing.lg),
          child: Button.secondary(
            onPressed:
                controller.items.any((item) => item.read && !item.actionable)
                ? controller.clearRead
                : null,
            child: const Text('Clear read notifications'),
          ),
        ),
      ],
      child: controller.items.isEmpty
          ? StateView.empty(
              title: 'No notifications',
              message:
                  controller.deliveryError ??
                  'Device connection requests and their results appear here.',
              icon: LucideIcons.bell,
            )
          : ListView(
              padding: const EdgeInsets.all(AppSpacing.lg),
              children: [
                if (controller.deliveryError case final error?) ...[
                  Alert(
                    title: const Text('System notifications'),
                    content: Text(error),
                  ),
                  const Gap(AppSpacing.lg),
                ],
                for (final item in controller.items) ...[
                  Basic(
                    leading: Icon(
                      item.actionable
                          ? LucideIcons.monitorSmartphone
                          : LucideIcons.info,
                    ),
                    title: Text(item.title),
                    subtitle: Text(item.peerName),
                    trailing: controller.canOpen(item)
                        ? Button.primary(
                            onPressed: () {
                              closeDrawer(context);
                              controller.open(item.id);
                            },
                            child: const Text('Review'),
                          )
                        : null,
                  ),
                  const Gap(AppSpacing.md),
                  const Divider(),
                  const Gap(AppSpacing.md),
                ],
              ],
            ),
    ),
  );
}
