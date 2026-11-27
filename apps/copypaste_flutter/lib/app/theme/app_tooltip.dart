import 'dart:async';

import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'app_motion.dart';

/// The application's tooltip: the stock tooltip surface with the stock
/// triggers, placed once when it opens.
///
/// The stock [Tooltip] follows its anchor by polling it on every frame, and
/// times its opening delay with an animation, so a resting pointer keeps the
/// whole window redrawing. A tooltip closes as soon as the pointer leaves its
/// anchor, so it never has to follow one, and a timer waits without frames.
class AppTooltip extends StatefulWidget {
  const AppTooltip({super.key, required this.tooltip, required this.child});

  final WidgetBuilder tooltip;
  final Widget child;

  @override
  State<AppTooltip> createState() => _AppTooltipState();
}

class _AppTooltipState extends State<AppTooltip> {
  final _controller = OverlayController();
  Timer? _opening;

  @override
  void dispose() {
    _opening?.cancel();
    _controller.dispose();
    super.dispose();
  }

  void _enter() => _opening ??= Timer(AppMotion.tooltipDelay, _open);

  void _open() {
    _opening = null;
    if (!mounted) return;
    unawaited(
      _controller.show<void>(
        context,
        const TooltipConfiguration(
          alignment: Alignment.topCenter,
          anchorAlignment: Alignment.bottomCenter,
          follow: false,
        ),
        builder: (context) => widget.tooltip(context),
        adaptive: false,
      ),
    );
  }

  void _leave() {
    _opening?.cancel();
    _opening = null;
    _controller.close();
  }

  @override
  Widget build(BuildContext context) {
    // Without a pointer to rest, a long press opens the tooltip.
    final touch = switch (Theme.of(context).platform) {
      TargetPlatform.android ||
      TargetPlatform.iOS ||
      TargetPlatform.fuchsia => true,
      _ => false,
    };
    return TapRegion(
      onTapOutside: (_) => _leave(),
      child: MouseRegion(
        hitTestBehavior: HitTestBehavior.deferToChild,
        onEnter: (_) => _enter(),
        onExit: (_) => _leave(),
        child: GestureDetector(
          onLongPressDown: touch ? (_) => _enter() : null,
          onLongPressCancel: touch ? _leave : null,
          onLongPressUp: touch ? _leave : null,
          child: widget.child,
        ),
      ),
    );
  }
}
