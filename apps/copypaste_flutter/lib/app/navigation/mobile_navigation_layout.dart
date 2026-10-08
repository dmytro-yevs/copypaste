import 'dart:math' as math;

import 'package:flutter/widgets.dart';

import '../theme/app_tokens.dart';

/// Allocates the Android Telegram main-tab geometry to the app's labels.
class MobileNavigationLayout {
  const MobileNavigationLayout._({
    required this.widths,
    required this.fontSize,
    required this.horizontalPadding,
    required this.itemHeight,
    required this.textDirection,
  });

  final List<double> widths;
  final double fontSize;
  final double horizontalPadding;
  final double itemHeight;
  final TextDirection textDirection;

  double get width =>
      widths.fold(0.0, (total, width) => total + width) + AppSpacing.xs * 2;
  double get height => itemHeight + AppSpacing.xs * 2;

  factory MobileNavigationLayout.resolve({
    required double availableWidth,
    required List<String> labels,
    required TextStyle textStyle,
    required TextScaler textScaler,
    required TextDirection textDirection,
  }) {
    final width = math.min(
      availableWidth,
      AppLayoutSize.mobileNavigationMaxWidth,
    );
    final available = math.max(0.0, width - AppSpacing.xs * 2);
    final enlarged =
        textScaler.scale(AppTypographySize.navigation) >
        AppTypographySize.navigation;
    var fontSize = AppTypographySize.navigation;
    var padding = AppSpacing.lg;
    var measured = <double>[];
    for (var pass = 0; pass < 3; pass++) {
      padding = [AppSpacing.lg, AppSpacing.sm, AppSpacing.xs][pass];
      fontSize = pass == 2 && !enlarged
          ? AppTypographySize.navigationCompact
          : AppTypographySize.navigation;
      measured = labels.map((label) {
        final painter = TextPainter(
          text: TextSpan(
            text: label,
            style: textStyle.copyWith(fontSize: fontSize),
          ),
          textScaler: textScaler,
          textDirection: textDirection,
          maxLines: 1,
        )..layout();
        final result = painter.width + padding * 2;
        painter.dispose();
        return result;
      }).toList();
      if (measured.fold(0.0, (sum, item) => sum + item) <= available) break;
    }
    final sum = measured.fold(0.0, (sum, item) => sum + item);
    final average = available / labels.length;
    var growing = measured.map((item) => item <= average).toList();
    var count = growing.where((item) => item).length;
    if (count == 0) {
      growing = List.filled(labels.length, true);
      count = labels.length;
    }
    final widths = [
      for (var index = 0; index < measured.length; index++)
        sum > available
            ? measured[index] * available / sum
            : measured[index] +
                  (growing[index] ? (available - sum) / count : 0),
    ];
    final scaledHeight =
        AppControlSize.navigationLabelHeight *
        textScaler.scale(fontSize) /
        fontSize;
    return MobileNavigationLayout._(
      widths: widths,
      fontSize: fontSize,
      horizontalPadding: padding,
      itemHeight:
          AppControlSize.touch +
          scaledHeight -
          AppControlSize.navigationLabelHeight,
      textDirection: textDirection,
    );
  }

  double _logicalCenterAt(int index) =>
      AppSpacing.xs +
      widths.take(index).fold(0.0, (sum, width) => sum + width) +
      widths[index] / 2;

  double centerAt(int index) => textDirection == TextDirection.rtl
      ? width - _logicalCenterAt(index)
      : _logicalCenterAt(index);

  double positionAt(double x) {
    if (textDirection == TextDirection.rtl) x = width - x;
    if (x <= _logicalCenterAt(0)) return 0;
    for (var index = 1; index < widths.length; index++) {
      if (x <= _logicalCenterAt(index)) {
        return index -
            1 +
            (x - _logicalCenterAt(index - 1)) /
                (_logicalCenterAt(index) - _logicalCenterAt(index - 1));
      }
    }
    return (widths.length - 1).toDouble();
  }
}
