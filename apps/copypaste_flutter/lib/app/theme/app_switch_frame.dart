import 'package:flutter/foundation.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../l10n/l10n.dart';
import 'app_motion.dart';
import 'app_theme.dart';
import 'app_tokens.dart';

/// Scopes the shared switch shape to a stock [Switch] and writes its state
/// beside it, so the state never depends on colour or thumb position alone.
///
/// The word is visual only; the frame reports the toggled and enabled state.
/// Both words are laid out, so toggling never moves the switch or its
/// neighbours. On touch platforms the whole 48 px target toggles the switch.
class AppSwitchFrame extends StatelessWidget {
  const AppSwitchFrame({
    super.key,
    required this.child,
    this.showState = true,
    this.stacked = false,
  });

  final Switch child;

  /// False where a more specific status already describes the same state.
  final bool showState;

  /// Puts the word under the switch where a row has no width to spare.
  final bool stacked;

  static const _touchPlatforms = {TargetPlatform.android, TargetPlatform.iOS};

  @override
  Widget build(BuildContext context) {
    // The same rule the stock switch uses to decide how it paints.
    final enabled = child.enabled ?? child.onChanged != null;
    final interactive = enabled && child.onChanged != null;
    Widget control = Stack(
      clipBehavior: Clip.none,
      children: [
        Theme(data: AppTheme.switchControlTheme(context), child: child),
        Positioned.fill(
          child: IgnorePointer(
            child: _SwitchMarks(value: child.value, enabled: enabled),
          ),
        ),
      ],
    );
    // A pressable row around the switch is already the larger target.
    if (_touchPlatforms.contains(defaultTargetPlatform) &&
        context.findAncestorWidgetOfExactType<Clickable>() == null) {
      control = GestureDetector(
        behavior: HitTestBehavior.opaque,
        excludeFromSemantics: true,
        onTap: interactive ? () => child.onChanged!(!child.value) : null,
        child: ConstrainedBox(
          constraints: const BoxConstraints(
            minWidth: AppControlSize.touch,
            minHeight: AppControlSize.touch,
          ),
          // The track keeps its end edge on the row's control line.
          child: Align(
            alignment: AlignmentDirectional.centerEnd,
            widthFactor: 1,
            heightFactor: 1,
            child: control,
          ),
        ),
      );
    }
    control = Semantics(
      toggled: child.value,
      enabled: interactive,
      child: control,
    );
    if (!showState) return control;
    final style = AppTheme.switchStateTextStyle(context);
    final state = Flexible(
      child: ExcludeSemantics(
        child: IndexedStack(
          index: child.value ? 0 : 1,
          alignment: AlignmentDirectional.centerEnd,
          children: [
            for (final word in [context.l10n.commonOn, context.l10n.commonOff])
              Text(
                word,
                style: style,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
          ],
        ),
      ),
    );
    return Flex(
      direction: stacked ? Axis.vertical : Axis.horizontal,
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: stacked
          ? CrossAxisAlignment.end
          : CrossAxisAlignment.center,
      children: stacked
          ? [control, const Gap(AppSpacing.xxs), state]
          : [state, const Gap(AppSpacing.sm), control],
    );
  }
}

/// Paints the on bar, the off ring and the track outline over the stock
/// track, cross-fading them while the thumb travels.
class _SwitchMarks extends StatelessWidget {
  const _SwitchMarks({required this.value, required this.enabled});

  final bool value;
  final bool enabled;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return AppMotion.progressTransition(
      context,
      value: value ? 1 : 0,
      builder: (context, on, _) => CustomPaint(
        painter: _SwitchMarksPainter(
          on: on,
          enabled: enabled,
          // The stock track insets its thumb by a quarter of the density gap.
          inset: theme.density.baseGap * theme.scaling / 4,
          scale: theme.scaling,
          textDirection: Directionality.of(context),
          colors: AppTheme.switchMarkColors(context),
        ),
      ),
    );
  }
}

class _SwitchMarksPainter extends CustomPainter {
  _SwitchMarksPainter({
    required this.on,
    required this.enabled,
    required this.inset,
    required this.scale,
    required this.textDirection,
    required this.colors,
  });

  // Mark geometry in the stock track's unscaled units (36 x 20).
  static const _barWidth = 1.5;
  static const _barHeight = 8.0;
  static const _ringDiameter = 6.0;
  static const _ringStroke = 1.2;
  static const _dash = 3.0;
  static const _dashGap = 2.0;
  static const _outline = 1.0;

  /// 1 when the switch is on, 0 when off, in between while it moves.
  final double on;
  final bool enabled;
  final double inset;
  final double scale;
  final TextDirection textDirection;
  final ({Color on, Color off, Color outline, Color disabled}) colors;

  @override
  void paint(Canvas canvas, Size size) {
    final track = RRect.fromRectAndRadius(
      Offset.zero & size,
      const Radius.circular(AppTheme.switchTrackRadius),
    ).deflate(_outline / 2);
    final stroke = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = _outline;
    if (!enabled) {
      stroke.color = colors.disabled;
      final dash = _dash * scale;
      final step = dash + _dashGap * scale;
      for (final metric in (Path()..addRRect(track)).computeMetrics()) {
        for (var at = 0.0; at < metric.length; at += step) {
          canvas.drawPath(metric.extractPath(at, at + dash), stroke);
        }
      }
      return;
    }
    final off = 1 - on;
    final thumb = size.height - inset * 2;
    // The on mark sits where the thumb rests when off, and the other way round.
    final start = _center(
      size,
      Rect.fromLTRB(inset, inset, size.width - inset - thumb, thumb + inset),
    );
    final end = _center(
      size,
      Rect.fromLTRB(inset + thumb, inset, size.width - inset, thumb + inset),
    );
    if (off > 0) {
      canvas.drawRRect(track, stroke..color = _fade(colors.outline, off));
      canvas.drawCircle(
        end,
        (_ringDiameter - _ringStroke) * scale / 2,
        Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = _ringStroke * scale
          ..color = _fade(colors.off, off),
      );
    }
    if (on > 0) {
      canvas.drawRect(
        Rect.fromCenter(
          center: start,
          width: _barWidth * scale,
          height: _barHeight * scale,
        ),
        Paint()..color = _fade(colors.on, on),
      );
    }
  }

  Offset _center(Size size, Rect gap) => textDirection == TextDirection.rtl
      ? Offset(size.width - gap.center.dx, gap.center.dy)
      : gap.center;

  static Color _fade(Color color, double amount) =>
      amount >= 1 ? color : color.withValues(alpha: color.a * amount);

  @override
  bool shouldRepaint(_SwitchMarksPainter old) =>
      old.on != on ||
      old.enabled != enabled ||
      old.inset != inset ||
      old.scale != scale ||
      old.textDirection != textDirection ||
      old.colors != colors;
}
