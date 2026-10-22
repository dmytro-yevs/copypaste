import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../app/theme/app_motion.dart';
import '../../app/theme/app_theme.dart';
import '../../app/theme/app_tokens.dart';

/// Paints transfer progress around an existing button without changing its
/// interaction or semantics.
class FileOperationsButtonProgress extends StatelessWidget {
  const FileOperationsButtonProgress({
    super.key,
    required this.child,
    required this.progress,
  });

  final Widget child;

  /// `null` represents an active transfer with an unknown total size.
  final double? progress;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final extent = AppTheme.controlHeight(context);
    final radius = theme.radiusMd;
    final trackColor = theme.colorScheme.border.withValues(alpha: 0.24);

    Widget paint(double value, {required bool staticBorder}) => IgnorePointer(
      child: CustomPaint(
        painter: _ButtonProgressPainter(
          progress: value,
          radius: radius,
          color: AppTheme.navigationAccent,
          trackColor: trackColor,
          staticBorder: staticBorder,
        ),
      ),
    );

    return SizedBox.square(
      dimension: extent,
      child: Stack(
        fit: StackFit.expand,
        children: [
          child,
          if (progress == null)
            paint(1, staticBorder: true)
          else
            AppMotion.progressTransition(
              context,
              value: progress!.clamp(0, 1).toDouble(),
              builder: (context, value, _) =>
                  paint(value, staticBorder: false),
            ),
        ],
      ),
    );
  }
}

class _ButtonProgressPainter extends CustomPainter {
  const _ButtonProgressPainter({
    required this.progress,
    required this.radius,
    required this.color,
    required this.trackColor,
    required this.staticBorder,
  });

  final double progress;
  final double radius;
  final Color color;
  final Color trackColor;
  final bool staticBorder;

  @override
  void paint(Canvas canvas, Size size) {
    const thickness = AppSpacing.xxs;
    final inset = thickness / 2;
    final cornerRadius = (radius - inset).clamp(0, double.infinity).toDouble();
    final path = Path()
      ..addRRect(
        RRect.fromRectAndRadius(
          Rect.fromLTWH(
            inset,
            inset,
            size.width - thickness,
            size.height - thickness,
          ),
          Radius.circular(cornerRadius),
        ),
      );
    final trackPaint = Paint()
      ..color = trackColor
      ..strokeWidth = thickness
      ..style = PaintingStyle.stroke;
    final progressPaint = Paint()
      ..color = color
      ..strokeWidth = thickness
      ..style = PaintingStyle.stroke
      ..strokeCap = StrokeCap.round;

    if (!staticBorder) canvas.drawPath(path, trackPaint);
    if (staticBorder) {
      canvas.drawPath(path, progressPaint);
      return;
    }

    for (final metric in path.computeMetrics()) {
      final end = metric.length * progress;
      if (end > 0) canvas.drawPath(metric.extractPath(0, end), progressPaint);
    }
  }

  @override
  bool shouldRepaint(covariant _ButtonProgressPainter oldDelegate) =>
      oldDelegate.progress != progress ||
      oldDelegate.radius != radius ||
      oldDelegate.color != color ||
      oldDelegate.trackColor != trackColor ||
      oldDelegate.staticBorder != staticBorder;
}
